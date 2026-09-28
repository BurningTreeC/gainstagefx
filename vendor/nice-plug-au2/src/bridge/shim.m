// AUv2 hosts discover Cocoa editors through the statically compiled
// Objective-C class metadata for AUCocoaUIBase. A class registered from Rust
// at runtime may pass AU validation but still not be instantiated by a host,
// so this small Objective-C shim remains the host-facing factory.
#import <AppKit/AppKit.h>
#import <AudioUnit/AUCocoaUIView.h>

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
