// Open GainStageFx's Audio Unit editor the way a host does, and report what
// arrives.
//
// `inproc` asks the component for its Cocoa view (kAudioUnitProperty_CocoaUI),
// which is the AUv2 path most hosts take. `outofproc` loads the unit into
// AUHostingService and asks AUAudioUnit for a view controller, which is how
// Logic Pro and GarageBand can open it. Either way audio is rendered on a
// second thread while the editor is up, as a host does.
//
// The main thread has to keep turning. A watchdog samples this process (and
// any AUHostingService) if it stops for WATCHDOG seconds. At the end every
// view's frame is printed and the window is captured and its pixels checked.
//
//   clang -fobjc-arc -framework AppKit -framework AudioToolbox \
//     -framework AVFoundation -framework CoreAudioKit \
//     -o au_editor_smoke tools/au_editor_smoke.m
//   ./au_editor_smoke inproc|outofproc OUT_DIR [SECONDS]
//
// The component must already be registered (installed in
// ~/Library/Audio/Plug-Ins/Components). Exit status: 0 the editor arrived,
// drew and the main thread ran throughout; 1 a check failed; 2 setup failed;
// 3 the main thread hung.

#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <AudioToolbox/AudioToolbox.h>
#import <AudioUnit/AUCocoaUIView.h>
#import <CoreAudioKit/CoreAudioKit.h>
#include <errno.h>
#include <mach/mach_time.h>
#include <math.h>
#include <pthread.h>
#include <spawn.h>
#include <stdatomic.h>
#include <stddef.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

static const AudioComponentDescription DESCRIPTION = {
    kAudioUnitType_Effect, 'GSfx', 'BrTC', 0, 0};
static const double RATE = 48000.0;
enum { FRAMES = 512 };
static const double WATCHDOG = 10.0;

static NSString *out_dir;
static NSString *mode;
static double started;
static mach_timebase_info_data_t timebase;
static _Atomic uint64_t last_beat;
static atomic_bool rendering;
// Set by completion handlers that may run on any thread.
static atomic_bool instantiated, answered;
static atomic_bool render_failed;

static double seconds(uint64_t ticks) {
    return (double)ticks * timebase.numer / timebase.denom / 1e9;
}

static double now(void) { return seconds(mach_absolute_time()); }

static void say(NSString *format, ...) NS_FORMAT_FUNCTION(1, 2);
static void say(NSString *format, ...) {
    va_list args;
    va_start(args, format);
    NSString *line = [[NSString alloc] initWithFormat:format arguments:args];
    va_end(args);
    fprintf(stdout, "[%s %7.3f] %s\n", mode.UTF8String, now() - started, line.UTF8String);
    fflush(stdout);
}

static void beat(void) { atomic_store(&last_beat, mach_absolute_time()); }

static NSString *path(NSString *name) {
    return [out_dir stringByAppendingPathComponent:[NSString stringWithFormat:@"%@-%@", mode, name]];
}

// posix_spawn rather than NSTask: the watchdog calls this while the main
// thread, and so the main run loop, is stuck.
static int run(const char *const argv[]) {
    pid_t pid;
    int status = posix_spawn(&pid, argv[0], NULL, NULL, (char *const *)argv, environ);
    if (status != 0) {
        fprintf(stdout, "cannot run %s: %s\n", argv[0], strerror(status));
        return -1;
    }
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {
    }
    return WIFEXITED(status) ? WEXITSTATUS(status) : -1;
}

static void sample_pid(pid_t pid, NSString *name) {
    NSString *file = path([NSString stringWithFormat:@"sample-%@-%d.txt", name, pid]);
    NSString *pid_text = [NSString stringWithFormat:@"%d", pid];
    // A hardened system service can only be sampled as root; the runner has
    // passwordless sudo, a workstation falls through to the plain call.
    const char *as_root[] = {"/usr/bin/sudo", "-n", "/usr/bin/sample", pid_text.UTF8String,
                             "2", "-file", file.UTF8String, NULL};
    if (run(as_root) != 0) {
        const char *plain[] = {"/usr/bin/sample", pid_text.UTF8String, "2", "-file",
                               file.UTF8String, NULL};
        run(plain);
    }
    fprintf(stdout, "sampled %s (%d) into %s\n", name.UTF8String, pid, file.UTF8String);
}

static void sample_everything(void) {
    sample_pid(getpid(), @"host");
    FILE *pgrep = popen("/usr/bin/pgrep -f AUHostingService", "r");
    if (pgrep == NULL) return;
    int pid;
    while (fscanf(pgrep, "%d", &pid) == 1) sample_pid(pid, @"AUHostingService");
    pclose(pgrep);
}

static void capture_screen(NSString *name) {
    NSString *file = path(name);
    const char *argv[] = {"/usr/sbin/screencapture", "-x", file.UTF8String, NULL};
    run(argv);
}

static void *watchdog(void *unused) {
    (void)unused;
    for (;;) {
        usleep(200000);
        double stalled = now() - seconds(atomic_load(&last_beat));
        if (stalled > WATCHDOG) {
            fprintf(stdout, "[%s] MAIN THREAD HUNG: no turn of the run loop for %.1f s\n",
                    mode.UTF8String, stalled);
            fflush(stdout);
            capture_screen(@"hung.png");
            sample_everything();
            fflush(stdout);
            _exit(3);
        }
    }
    return NULL;
}

// Run the main thread as an application would, delivering events and firing
// timers in the default mode, for `duration` or until `done` says so.
static BOOL pump(double duration, BOOL (^done)(void)) {
    double end = now() + duration;
    while (now() < end) {
        if (done != nil && done()) return YES;
        @autoreleasepool {
            NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                                untilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]
                                                   inMode:NSDefaultRunLoopMode
                                                  dequeue:YES];
            if (event != nil) [NSApp sendEvent:event];
            [NSApp updateWindows];
        }
        beat();
    }
    return done != nil && done();
}

// A quiet 110 Hz sine, the same into every channel.
static void fill(AudioBufferList *list, UInt32 frames) {
    static float scratch[2][4096];
    static double phase;
    for (UInt32 b = 0; b < list->mNumberBuffers; b++) {
        if (list->mBuffers[b].mData == NULL && b < 2) list->mBuffers[b].mData = scratch[b];
        list->mBuffers[b].mDataByteSize = frames * sizeof(float);
    }
    for (UInt32 i = 0; i < frames; i++) {
        float value = 0.1f * (float)sin(phase);
        phase = fmod(phase + 2.0 * M_PI * 110.0 / RATE, 2.0 * M_PI);
        for (UInt32 b = 0; b < list->mNumberBuffers; b++) {
            if (list->mBuffers[b].mData != NULL) ((float *)list->mBuffers[b].mData)[i] = value;
        }
    }
}

static OSStatus feed(void *ref, AudioUnitRenderActionFlags *flags, const AudioTimeStamp *time,
                     UInt32 bus, UInt32 frames, AudioBufferList *list) {
    (void)ref, (void)flags, (void)time, (void)bus;
    fill(list, frames);
    return noErr;
}

static AudioBufferList *output_list(void) {
    static float left[FRAMES], right[FRAMES];
    static AudioBufferList *list;
    if (list == NULL) list = calloc(1, offsetof(AudioBufferList, mBuffers) + 2 * sizeof(AudioBuffer));
    list->mNumberBuffers = 2;
    list->mBuffers[0] = (AudioBuffer){1, sizeof left, left};
    list->mBuffers[1] = (AudioBuffer){1, sizeof right, right};
    return list;
}

// Call `render` at the rate a device would, and say how it went.
static void render_like_a_device(OSStatus (^render)(const AudioTimeStamp *)) {
    AudioTimeStamp time = {0};
    time.mFlags = kAudioTimeStampSampleTimeValid;
    unsigned long long calls = 0, failed = 0;
    OSStatus first_failure = noErr;
    double worst = 0, period = FRAMES / RATE;
    while (atomic_load(&rendering)) {
        double began = now();
        OSStatus status = render(&time);
        double took = now() - began;
        if (took > worst) worst = took;
        if (status != noErr) {
            if (failed++ == 0) first_failure = status;
        }
        calls++;
        time.mSampleTime += FRAMES;
        if (took < period) usleep((useconds_t)((period - took) * 1e6));
    }
    say(@"%@rendered %llu blocks, %llu failed (first %d), slowest %.2f ms",
        failed > 0 ? @"FAIL: " : @"", calls, failed, (int)first_failure, worst * 1e3);
    if (failed > 0) atomic_store(&render_failed, true);
}

// Whether this machine offers the pixel format baseview asks for: OpenGL 3.2
// Core, accelerated. A virtual machine's paravirtualised GPU may not, and then
// an editor cannot open here however correct it is.
static BOOL accelerated_opengl(void) {
    NSOpenGLPixelFormatAttribute attributes[] = {
        NSOpenGLPFAOpenGLProfile, NSOpenGLProfileVersion3_2Core,
        NSOpenGLPFAColorSize, 24, NSOpenGLPFAAlphaSize, 8,
        NSOpenGLPFADepthSize, 24, NSOpenGLPFAStencilSize, 8,
        NSOpenGLPFAAccelerated, NSOpenGLPFADoubleBuffer, 0};
    return [[NSOpenGLPixelFormat alloc] initWithAttributes:attributes] != nil;
}

static BOOL has_subview_of_class(NSView *view, Class kind) {
    if ([view isKindOfClass:kind]) return YES;
    for (NSView *subview in view.subviews) {
        if (has_subview_of_class(subview, kind)) return YES;
    }
    return NO;
}

static void dump(NSView *view, int depth) {
    NSRect frame = view.frame;
    say(@"%*s%@ %.0fx%.0f at (%.0f,%.0f)%@%@%@", depth * 2, "", NSStringFromClass(view.class),
        frame.size.width, frame.size.height, frame.origin.x, frame.origin.y,
        view.hidden ? @" hidden" : @"", view.wantsLayer ? @" layer" : @"",
        view.window == nil ? @" no-window" : @"");
    for (NSView *subview in view.subviews) dump(subview, depth + 1);
}

// Distinct colours on a 64 x 64 grid over the image below its top `skip`
// pixels (the title bar), and the share of samples that are not black.
static NSUInteger colours(NSString *file, NSInteger skip, double *lit_share) {
    *lit_share = 0;
    NSData *data = [NSData dataWithContentsOfFile:file];
    NSBitmapImageRep *image = data != nil ? [NSBitmapImageRep imageRepWithData:data] : nil;
    if (image == nil) return 0;
    NSMutableSet *seen = [NSMutableSet set];
    NSInteger width = image.pixelsWide, height = image.pixelsHigh, total = 0, lit = 0;
    for (NSInteger y = MIN(skip, height); y < height; y += MAX(1, (height - skip) / 64)) {
        for (NSInteger x = 0; x < width; x += MAX(1, width / 64)) {
            NSColor *colour = [[image colorAtX:x y:y] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            long r = lround(colour.redComponent * 255), g = lround(colour.greenComponent * 255),
                 b = lround(colour.blueComponent * 255);
            [seen addObject:@((r << 16) | (g << 8) | b)];
            total++;
            if (r + g + b > 24) lit++;
        }
    }
    *lit_share = total > 0 ? (double)lit / total : 0;
    return seen.count;
}

// Follows the editor's frame the way a host's plug-in window does.
@interface Host : NSObject
@property(strong) NSWindow *window;
@property(strong) NSView *editor;
@property NSUInteger resizes;
@end

@implementation Host
- (void)editorFrameChanged:(NSNotification *)note {
    (void)note;
    NSSize size = self.editor.frame.size;
    self.resizes++;
    say(@"editor frame changed to %.0fx%.0f", size.width, size.height);
    if (self.resizes > 50) {
        say(@"FAIL: the editor keeps resizing itself");
        return;
    }
    if (!NSEqualSizes(self.window.contentView.frame.size, size)) [self.window setContentSize:size];
}

- (void)open:(NSView *)editor {
    self.editor = editor;
    NSSize size = editor.frame.size;
    NSRect rect = NSMakeRect(0, 0, MAX(size.width, 64), MAX(size.height, 64));
    self.window = [[NSWindow alloc] initWithContentRect:rect
                                              styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
    self.window.releasedWhenClosed = NO;
    self.window.title = [NSString stringWithFormat:@"GainStageFx (%@)", mode];
    [self.window center];
    editor.postsFrameChangedNotifications = YES;
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(editorFrameChanged:)
                                                 name:NSViewFrameDidChangeNotification
                                               object:editor];
    say(@"window backing scale %.1f", self.window.backingScaleFactor);
    double began = now();
    beat();
    [self.window.contentView addSubview:editor];
    beat();
    say(@"adding the editor to the window took %.3f s", now() - began);
    [self.window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

- (void)close {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    double began = now();
    [self.editor removeFromSuperview];
    [self.window close];
    say(@"closing the editor took %.3f s", now() - began);
    self.editor = nil;
    self.window = nil;
}
@end

static int report(Host *host, BOOL in_this_process) {
    int failures = 0;
    NSWindow *window = host.window;
    NSSize content = window.contentView.frame.size;
    say(@"window content %.0fx%.0f, editor %.0fx%.0f, scale %.1f", content.width, content.height,
        host.editor.frame.size.width, host.editor.frame.size.height, window.backingScaleFactor);
    dump(window.contentView, 0);
    if (host.editor.frame.size.width < 200 || host.editor.frame.size.height < 100) {
        say(@"FAIL: the editor is %.0fx%.0f", host.editor.frame.size.width,
            host.editor.frame.size.height);
        failures++;
    }

    NSString *file = path(@"window.png");
    NSString *window_id = [NSString stringWithFormat:@"-l%ld", (long)window.windowNumber];
    const char *argv[] = {"/usr/sbin/screencapture", "-x", "-o", window_id.UTF8String, file.UTF8String, NULL};
    run(argv);
    capture_screen(@"screen.png");
    BOOL opengl = accelerated_opengl();
    if (in_this_process) {
        BOOL spawned = has_subview_of_class(host.editor, NSOpenGLView.class);
        say(@"the editor's OpenGL view is %@", spawned ? @"there" : @"missing");
        if (!spawned && opengl) {
            say(@"FAIL: accelerated OpenGL is available and the editor did not open");
            failures++;
        }
    }
    // The title bar is window height less content height, in pixels.
    NSInteger title = (NSInteger)lround((window.frame.size.height - content.height) *
                                        window.backingScaleFactor);
    double lit = 0;
    NSUInteger seen = colours(file, title, &lit);
    say(@"editor capture: %lu colours, %.0f%% not black", (unsigned long)seen, lit * 100);
    if (seen == 0) {
        say(@"no window capture (screen recording not permitted?)");
    } else if (lit < 0.5) {
        if (opengl) {
            say(@"FAIL: the editor did not draw");
            failures++;
        } else {
            say(@"not checked: no accelerated OpenGL on this machine, so nothing can draw");
        }
    }
    return failures;
}

// The AUv2 way: the component names a Cocoa view factory class in a bundle.
// Returns the number of failed checks.
static int open_cocoa_view(AudioUnit unit, double duration) {
    UInt32 size = 0;
    Boolean writable = false;
    OSStatus status = AudioUnitGetPropertyInfo(unit, kAudioUnitProperty_CocoaUI, kAudioUnitScope_Global, 0,
                                      &size, &writable);
    if (status != noErr || size < sizeof(AudioUnitCocoaViewInfo)) {
        say(@"FAIL: no Cocoa view (%d, %u bytes)", (int)status, (unsigned)size);
        return 1;
    }
    AudioUnitCocoaViewInfo *info = malloc(size);
    status = AudioUnitGetProperty(unit, kAudioUnitProperty_CocoaUI, kAudioUnitScope_Global, 0, info, &size);
    if (status != noErr) {
        say(@"FAIL: kAudioUnitProperty_CocoaUI: %d", (int)status);
        return 1;
    }
    NSURL *bundle_url = (__bridge_transfer NSURL *)info->mCocoaAUViewBundleLocation;
    NSString *class_name = (__bridge_transfer NSString *)info->mCocoaAUViewClass[0];
    free(info);
    say(@"Cocoa view %@ in %@", class_name, bundle_url.path);
    NSBundle *bundle = [NSBundle bundleWithURL:bundle_url];
    Class factory_class = [bundle classNamed:class_name];
    if (factory_class == nil) {
        say(@"FAIL: %@ is not in the bundle", class_name);
        return 1;
    }
    id<AUCocoaUIBase> factory = [[factory_class alloc] init];
    double began = now();
    // Logic passes the size it last showed; nothing yet on a first open.
    NSView *editor = [factory uiViewForAudioUnit:unit withSize:NSZeroSize];
    beat();
    say(@"uiViewForAudioUnit took %.3f s and returned %@ %.0fx%.0f", now() - began,
        editor != nil ? NSStringFromClass(editor.class) : @"nil", editor.frame.size.width,
        editor.frame.size.height);
    if (editor == nil) return 1;

    Host *host = [[Host alloc] init];
    [host open:editor];
    pump(duration, nil);
    int failures = report(host, YES);

    // A second open, as when the user closes and reopens the plug-in window.
    [host close];
    pump(1.0, nil);
    editor = [factory uiViewForAudioUnit:unit withSize:NSZeroSize];
    say(@"reopened: %@ %.0fx%.0f", editor != nil ? NSStringFromClass(editor.class) : @"nil",
        editor.frame.size.width, editor.frame.size.height);
    if (editor != nil) {
        [host open:editor];
        pump(2.0, nil);
        say(@"reopened editor %.0fx%.0f", host.editor.frame.size.width, host.editor.frame.size.height);
        [host close];
        editor = nil;
    }
    pump(1.0, nil);

    return failures;
}

static int in_process(double duration) {
    AudioComponent component = AudioComponentFindNext(NULL, &DESCRIPTION);
    if (component == NULL) {
        say(@"aufx GSfx BrTC is not registered");
        return 2;
    }
    AudioUnit unit = NULL;
    OSStatus status = AudioComponentInstanceNew(component, &unit);
    if (status != noErr) {
        say(@"AudioComponentInstanceNew: %d", (int)status);
        return 2;
    }

    AudioStreamBasicDescription format = {0};
    format.mSampleRate = RATE;
    format.mFormatID = kAudioFormatLinearPCM;
    format.mFormatFlags = kAudioFormatFlagsNativeFloatPacked | kAudioFormatFlagIsNonInterleaved;
    format.mBytesPerPacket = format.mBytesPerFrame = sizeof(float);
    format.mFramesPerPacket = 1;
    format.mChannelsPerFrame = 2;
    format.mBitsPerChannel = 32;
    for (AudioUnitScope scope = kAudioUnitScope_Input; scope <= kAudioUnitScope_Output; scope++) {
        status = AudioUnitSetProperty(unit, kAudioUnitProperty_StreamFormat, scope, 0, &format, sizeof format);
        if (status != noErr) say(@"stream format (scope %u): %d", (unsigned)scope, (int)status);
    }
    UInt32 maximum = 1024;
    AudioUnitSetProperty(unit, kAudioUnitProperty_MaximumFramesPerSlice, kAudioUnitScope_Global, 0,
                         &maximum, sizeof maximum);
    AURenderCallbackStruct callback = {feed, NULL};
    AudioUnitSetProperty(unit, kAudioUnitProperty_SetRenderCallback, kAudioUnitScope_Input, 0,
                         &callback, sizeof callback);

    double began = now();
    status = AudioUnitInitialize(unit);
    beat();
    say(@"AudioUnitInitialize: %d in %.3f s", (int)status, now() - began);
    if (status != noErr) return 2;

    atomic_store(&rendering, true);
    NSThread *renderer = [[NSThread alloc] initWithBlock:^{
      render_like_a_device(^OSStatus(const AudioTimeStamp *time) {
        AudioUnitRenderActionFlags flags = 0;
        return AudioUnitRender(unit, &flags, time, 0, FRAMES, output_list());
      });
    }];
    renderer.qualityOfService = NSQualityOfServiceUserInteractive;
    [renderer start];

    int failures = open_cocoa_view(unit, duration);
    atomic_store(&rendering, false);
    pump(0.5, nil);
    began = now();
    AudioUnitUninitialize(unit);
    AudioComponentInstanceDispose(unit);
    say(@"disposed in %.3f s", now() - began);
    return failures > 0 ? 1 : 0;
}

static int out_of_process(double duration) {
    __block AUAudioUnit *unit = nil;
    __block NSError *failed = nil;
    double began = now();
    [AUAudioUnit instantiateWithComponentDescription:DESCRIPTION
                                             options:kAudioComponentInstantiation_LoadOutOfProcess
                                   completionHandler:^(AUAudioUnit *made, NSError *failure) {
                                     unit = made;
                                     failed = failure;
                                     atomic_store(&instantiated, true);
                                   }];
    if (!pump(30.0, ^BOOL { return atomic_load(&instantiated); })) {
        say(@"FAIL: out-of-process instantiation did not complete");
        return 1;
    }
    say(@"instantiated out of process in %.3f s: %@", now() - began,
        unit != nil ? NSStringFromClass(unit.class) : failed.description);
    if (unit == nil) return 2;

    NSError *error = nil;
    AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:RATE channels:2];
    if (unit.inputBusses.count > 0 && ![unit.inputBusses[0] setFormat:format error:&error])
        say(@"input format: %@", error);
    if (unit.outputBusses.count > 0 && ![unit.outputBusses[0] setFormat:format error:&error])
        say(@"output format: %@", error);
    unit.maximumFramesToRender = 1024;
    began = now();
    BOOL allocated = [unit allocateRenderResourcesAndReturnError:&error];
    beat();
    say(@"allocateRenderResources: %@ in %.3f s", allocated ? @"ok" : error.description, now() - began);

    if (allocated) {
        AURenderBlock render = unit.renderBlock;
        AURenderPullInputBlock pull =
            ^AUAudioUnitStatus(AudioUnitRenderActionFlags *flags, const AudioTimeStamp *time,
                               AUAudioFrameCount frames, NSInteger bus, AudioBufferList *list) {
              (void)flags, (void)time, (void)bus;
              fill(list, frames);
              return noErr;
            };
        atomic_store(&rendering, true);
        NSThread *renderer = [[NSThread alloc] initWithBlock:^{
          render_like_a_device(^OSStatus(const AudioTimeStamp *time) {
            AudioUnitRenderActionFlags flags = 0;
            return render(&flags, time, FRAMES, 0, output_list(), pull);
          });
        }];
        renderer.qualityOfService = NSQualityOfServiceUserInteractive;
        [renderer start];
    }

    __block NSViewController *controller = nil;
    began = now();
    [unit requestViewControllerWithCompletionHandler:^(AUViewControllerBase *made) {
      controller = made;
      atomic_store(&answered, true);
    }];
    if (!pump(30.0, ^BOOL { return atomic_load(&answered); })) {
        say(@"FAIL: requestViewController did not answer within 30 s");
        sample_everything();
        capture_screen(@"no-view.png");
        return 1;
    }
    say(@"requestViewController took %.3f s and returned %@", now() - began,
        controller != nil ? NSStringFromClass(controller.class) : @"nil");
    if (controller == nil) return 1;
    NSView *editor = controller.view;
    say(@"preferred content size %.0fx%.0f, view %.0fx%.0f", controller.preferredContentSize.width,
        controller.preferredContentSize.height, editor.frame.size.width, editor.frame.size.height);
    if (NSIsEmptyRect(editor.frame) && controller.preferredContentSize.width > 0)
        [editor setFrameSize:controller.preferredContentSize];

    Host *host = [[Host alloc] init];
    [host open:editor];
    pump(duration, nil);
    say(@"preferred content size now %.0fx%.0f", controller.preferredContentSize.width,
        controller.preferredContentSize.height);
    int failures = report(host, NO);
    // The editor runs in the service, so a hang there leaves this main thread
    // free: sample it whatever happened.
    sample_everything();

    [host close];
    controller = nil;
    atomic_store(&rendering, false);
    pump(1.0, nil);
    [unit deallocateRenderResources];
    unit = nil;
    return failures > 0 ? 1 : 0;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        mach_timebase_info(&timebase);
        started = now();
        beat();
        if (argc < 3 || (strcmp(argv[1], "inproc") != 0 && strcmp(argv[1], "outofproc") != 0)) {
            fprintf(stderr, "usage: %s inproc|outofproc OUT_DIR [SECONDS]\n", argv[0]);
            return 2;
        }
        mode = @(argv[1]);
        out_dir = @(argv[2]);
        double duration = argc > 3 ? atof(argv[3]) : 6.0;
        [[NSFileManager defaultManager] createDirectoryAtPath:out_dir
                                  withIntermediateDirectories:YES
                                                   attributes:nil
                                                        error:NULL];

        pthread_t thread;
        pthread_create(&thread, NULL, watchdog, NULL);
        pthread_detach(thread);

        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        [NSApp finishLaunching];
        beat();

        say(@"accelerated OpenGL 3.2 Core: %@", accelerated_opengl() ? @"yes" : @"no");
        int status = strcmp(argv[1], "inproc") == 0 ? in_process(duration) : out_of_process(duration);
        if (status == 0 && atomic_load(&render_failed)) status = 1;
        say(@"%s", status == 0 ? "PASS" : "FAIL");
        return status;
    }
}
