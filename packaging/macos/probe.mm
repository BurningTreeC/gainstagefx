// Validation host only; never linked into any shipped plugin.
// Reads the actual CLAP parameter model and exercises Apple's AU hosting API.
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#include <clap/clap.h>
#include <algorithm>
#include <cmath>
#include <cstring>
#include <dlfcn.h>
#include <iostream>
#include <stdexcept>
#include <vector>

static void check(bool ok, const char *message) {
    if (!ok) throw std::runtime_error(message);
}

static OSType fourcc(NSString *string) {
    const unsigned char *p = (const unsigned char *)string.UTF8String;
    check(strlen((const char *)p) == 4, "Invalid FourCC");
    return (p[0] << 24) | (p[1] << 16) | (p[2] << 8) | p[3];
}

static bool pendingCallback = false;
static const void *hostExtension(const clap_host_t *, const char *) { return nullptr; }
static void clapHostIgnoreRequest(const clap_host_t *) {}
static void callback(const clap_host_t *) { pendingCallback = true; }

struct ClapReference {
    void *library = nullptr;
    const clap_plugin_entry_t *entry = nullptr;
    const clap_plugin_t *plugin = nullptr;
    const clap_plugin_params_t *params = nullptr;
    clap_host_t host{CLAP_VERSION, nullptr, "BurningTreeC AU validation", "BurningTreeC",
                     "https://github.com/BurningTreeC", "1", hostExtension,
                     clapHostIgnoreRequest, clapHostIgnoreRequest, callback};
    std::vector<clap_param_info_t> infos;

    ClapReference(NSString *bundle, NSString *identifier) {
        NSBundle *clapBundle = [NSBundle bundleWithPath:bundle];
        library = dlopen(clapBundle.executablePath.fileSystemRepresentation, RTLD_LOCAL | RTLD_NOW);
        check(library != nullptr, "Cannot dlopen reference CLAP");
        entry = (const clap_plugin_entry_t *)dlsym(library, "clap_entry");
        check(entry && entry->init(bundle.fileSystemRepresentation), "Cannot initialize reference CLAP");
        auto factory = (const clap_plugin_factory_t *)entry->get_factory(CLAP_PLUGIN_FACTORY_ID);
        check(factory != nullptr, "Missing CLAP plugin factory");
        check(factory->get_plugin_count(factory) == 1, "Expected one CLAP descriptor");
        const auto *desc = factory->get_plugin_descriptor(factory, 0);
        check(strcmp(desc->id, identifier.UTF8String) == 0, "Wrong embedded CLAP ID");
        plugin = factory->create_plugin(factory, &host, identifier.UTF8String);
        check(plugin && plugin->init(plugin), "Cannot create reference CLAP instance");
        params = (const clap_plugin_params_t *)plugin->get_extension(plugin, CLAP_EXT_PARAMS);
        check(params != nullptr, "Missing CLAP params");
        for (uint32_t i = 0; i < params->count(plugin); ++i) {
            clap_param_info_t info{};
            check(params->get_info(plugin, i, &info), "Cannot read CLAP parameter");
            infos.push_back(info);
        }
        if (pendingCallback) { pendingCallback = false; plugin->on_main_thread(plugin); }
    }
    ~ClapReference() {
        if (plugin) plugin->destroy(plugin);
        if (entry) entry->deinit();
        if (library) dlclose(library);
    }
};

static AUAudioUnit *instantiate(AudioComponentDescription description) {
    __block AUAudioUnit *unit = nil;
    __block NSError *failure = nil;
    __block bool complete = false;
    [AUAudioUnit instantiateWithComponentDescription:description options:0
        completionHandler:^(AUAudioUnit *result, NSError *error) {
            unit = result; failure = error; complete = true;
        }];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:30];
    while (!complete && deadline.timeIntervalSinceNow > 0)
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    if (failure) std::cerr << failure.description.UTF8String << "\n";
    check(complete && unit, "AU instantiation failed or timed out");
    return unit;
}

static bool near(double a, double b) {
    return std::abs(a - b) <= 1e-5 * std::max(1.0, std::max(std::abs(a), std::abs(b)));
}

static void settle() {
    [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.1]];
}

int main(int argc, char **argv) {
    @autoreleasepool {
        try {
            check(argc == 6, "probe manifest.json package auv2|auv3 reference.clap report.json");
            NSDictionary *manifest = [NSJSONSerialization JSONObjectWithData:
                [NSData dataWithContentsOfFile:@(argv[1])] options:0 error:nil];
            NSDictionary *plugin = nil;
            for (NSDictionary *p in manifest[@"plugins"])
                if ([p[@"package"] isEqualToString:@(argv[2])]) plugin = p;
            check(plugin != nil, "Unknown plugin in manifest");
            bool v3 = strcmp(argv[3], "auv3") == 0;
            AudioComponentDescription desc{};
            desc.componentType = fourcc(plugin[@"type"]);
            desc.componentSubType = fourcc(plugin[@"subtype"]);
            desc.componentManufacturer = fourcc(plugin[@"manufacturer"]);
            desc.componentFlagsMask = kAudioComponentFlag_IsV3AudioUnit;
            desc.componentFlags = v3 ? kAudioComponentFlag_IsV3AudioUnit : 0;
            AudioComponent component = AudioComponentFindNext(nullptr, &desc);
            check(component != nullptr, "Requested AU version is not registered");
            AudioComponentDescription actual{};
            check(AudioComponentGetDescription(component, &actual) == noErr, "Cannot inspect AU description");
            check(bool(actual.componentFlags & kAudioComponentFlag_IsV3AudioUnit) == v3,
                  "Host selected the wrong AU version");
            AUAudioUnit *unit = instantiate(desc);
            check(bool(unit.componentDescription.componentFlags & kAudioComponentFlag_IsV3AudioUnit) == v3,
                  "Instantiated AU version differs from request");
            ClapReference clap(@(argv[4]), plugin[@"clap_id"]);
            NSArray<AUParameter *> *parameters = unit.parameterTree.allParameters;
            size_t expectedCount = 0;
            NSMutableArray *parameterReport = [NSMutableArray array];
            NSMutableDictionary *changedValues = [NSMutableDictionary dictionary];
            for (const auto &info : clap.infos) {
                if (v3 && (info.flags & CLAP_PARAM_IS_HIDDEN)) continue;
                ++expectedCount;
                AUParameter *parameter = [unit.parameterTree parameterWithAddress:info.id];
                check(parameter != nil, "CLAP parameter ID missing from AU");
                check([parameter.displayName isEqualToString:@(info.name)], "AU parameter name mismatch");
                check(near(parameter.minValue, info.min_value) && near(parameter.maxValue, info.max_value),
                      "AU parameter range mismatch");
                check(near(parameter.value, info.default_value), "AU parameter default mismatch");
                bool writable = !(info.flags & (CLAP_PARAM_IS_READONLY | CLAP_PARAM_IS_HIDDEN));
                if (v3) writable = writable && (info.flags & CLAP_PARAM_IS_AUTOMATABLE);
                check(bool(parameter.flags & kAudioUnitParameterFlag_IsWritable) == writable,
                      "AU parameter writable/automation flags mismatch");
                [parameterReport addObject:@{@"id": @(info.id), @"name": @(info.name),
                    @"min": @(info.min_value), @"max": @(info.max_value),
                    @"default": @(info.default_value), @"clap_flags": @(info.flags)}];
                if (writable) {
                    double value = info.min_value + (info.max_value - info.min_value) * 0.37;
                    if (info.flags & CLAP_PARAM_IS_STEPPED) value = std::round(value);
                    parameter.value = (AUValue)value;
                    changedValues[@(info.id)] = @((AUValue)value);
                }
            }
            check(parameters.count == expectedCount, "AU/CLAP parameter count mismatch");
            settle();
            for (NSNumber *address in changedValues)
                check(near([unit.parameterTree parameterWithAddress:address.unsignedLongLongValue].value,
                           [changedValues[address] doubleValue]), "Host parameter update was not applied");
            NSMutableDictionary *values = [NSMutableDictionary dictionary];
            for (AUParameter *parameter in parameters) values[@(parameter.address)] = @(parameter.value);
            NSDictionary *state = unit.fullState;
            check(state != nil, "AU has no restorable state");
            NSError *error = nil;
            NSData *serialized = [NSPropertyListSerialization dataWithPropertyList:state
                format:NSPropertyListBinaryFormat_v1_0 options:0 error:&error];
            check(serialized != nil, "AU state is not serializable");
            unit = nil;
            settle();
            unit = instantiate(desc);
            unit.fullState = [NSPropertyListSerialization propertyListWithData:serialized
                options:NSPropertyListImmutable format:nullptr error:&error];
            settle();
            for (AUParameter *parameter in unit.parameterTree.allParameters)
                check(near(parameter.value, [values[@(parameter.address)] doubleValue]),
                      "Parameter did not survive serialized state restoration");
            // Restore reference defaults before rendering so all probes have
            // comparable settings. This is a finite-output/layout smoke test,
            // not a claim of sample-exact DSP equivalence or a realtime audit.
            for (const auto &info : clap.infos)
                [unit.parameterTree parameterWithAddress:info.id].value = (AUValue)info.default_value;
            settle();
            check(unit.inputBusses.count == 1 && unit.outputBusses.count == 1, "Unexpected AU bus count");
            NSMutableArray *renders = [NSMutableArray array];
            for (double rate : {44100.0, 48000.0, 96000.0}) {
                for (AVAudioChannelCount channels : {1u, 2u}) {
                    for (AUAudioFrameCount frames : {32u, 128u, 512u}) {
                        AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:rate channels:channels];
                        check([unit.inputBusses[0] setFormat:format error:&error], "AU rejected advertised input layout/rate");
                        check([unit.outputBusses[0] setFormat:format error:&error], "AU rejected advertised output layout/rate");
                        unit.maximumFramesToRender = frames;
                        if (![unit allocateRenderResourcesAndReturnError:&error]) {
                            std::cerr << error.description.UTF8String << "\n";
                            check(false, "AU resource allocation failed");
                        }
                        AVAudioPCMBuffer *output = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:frames];
                        output.frameLength = frames;
                        AURenderPullInputBlock input = ^AUAudioUnitStatus(AudioUnitRenderActionFlags *,
                            const AudioTimeStamp *stamp, AUAudioFrameCount count, NSInteger, AudioBufferList *data) {
                            for (UInt32 c = 0; c < data->mNumberBuffers; ++c) {
                                float *samples = (float *)data->mBuffers[c].mData;
                                if (!samples) return kAudioUnitErr_InvalidPropertyValue;
                                for (UInt32 n = 0; n < count; ++n)
                                    samples[n] = 0.01f * std::sin(2.0 * M_PI * 440.0 * (stamp->mSampleTime + n) / rate);
                                data->mBuffers[c].mDataByteSize = count * sizeof(float);
                            }
                            return noErr;
                        };
                        for (int block = 0; block < 8; ++block) {
                            AudioTimeStamp stamp{};
                            stamp.mFlags = kAudioTimeStampSampleTimeValid;
                            stamp.mSampleTime = block * frames;
                            AudioUnitRenderActionFlags flags = 0;
                            check(unit.renderBlock(&flags, &stamp, frames, 0, output.mutableAudioBufferList, input) == noErr,
                                  "AU render failed");
                            for (unsigned c = 0; c < channels; ++c)
                                for (unsigned n = 0; n < frames; ++n)
                                    check(std::isfinite(output.floatChannelData[c][n]), "AU emitted non-finite samples");
                        }
                        [unit deallocateRenderResources];
                        [renders addObject:@{@"rate": @(rate), @"channels": @(channels), @"frames": @(frames)}];
                    }
                }
            }
            NSDictionary *report = @{@"package": plugin[@"package"], @"format": @(argv[3]),
                @"parameters": parameterReport, @"state_roundtrip": @"passed", @"renders": renders,
                @"gui": @"manual test required", @"gestures_and_live_automation": @"manual test required",
                @"dsp_equivalence": @"not measured"};
            NSData *json = [NSJSONSerialization dataWithJSONObject:report options:NSJSONWritingPrettyPrinted error:&error];
            check([json writeToFile:@(argv[5]) atomically:YES], "Cannot write probe report");
            std::cout << "PASS " << argv[2] << " " << argv[3] << ": " << expectedCount
                      << " parameters, serialized state, 18 render configurations\n";
            return 0;
        } catch (const std::exception &error) {
            std::cerr << "FAIL: " << error.what() << "\n";
            return 1;
        }
    }
}
