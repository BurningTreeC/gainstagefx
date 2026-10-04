// AUv2 hosts discover Cocoa editors through the statically compiled
// Objective-C class metadata for AUCocoaUIBase. A class registered from Rust
// at runtime may pass AU validation but still not be instantiated by a host,
// so this small Objective-C shim remains the host-facing factory.
#import <AppKit/AppKit.h>
#import <AudioUnit/AUCocoaUIView.h>
#import <objc/runtime.h>

// Rust owns the editor implementation and returns the actual NSView.
extern NSView* nice_au2_create_cocoa_view(AudioUnit audioUnit);

@interface NiceAu2CocoaViewFactory : NSObject <AUCocoaUIBase>
@end

@implementation NiceAu2CocoaViewFactory

- (unsigned)interfaceVersion {
    return 0;
}

// Keep the Objective-C entry point required by AUv2 hosts, then delegate all
// editor creation and lifecycle work to Rust.
- (NSView*)uiViewForAudioUnit:(AudioUnit)audioUnit withSize:(NSSize)preferredSize {
    (void)preferredSize;
    return nice_au2_create_cocoa_view(audioUnit);
}

@end

// The name kAudioUnitProperty_CocoaUI gives the host, read from the class
// itself. This is also what puts the class in the plugin at all: the shim is a
// static archive, and the linker takes an archive member only for a symbol
// something else needs. Nothing in Rust called into this file, so the class
// was left out and every host's lookup of it came back nil -- an empty 1x1
// view out of process (Logic Pro, GarageBand), no editor in process.
const char* nice_au2_cocoa_view_class_name(void) {
    return class_getName([NiceAu2CocoaViewFactory class]);
}
