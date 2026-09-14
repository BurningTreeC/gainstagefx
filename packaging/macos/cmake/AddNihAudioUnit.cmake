function(add_nih_clap_audio_unit plugin)
    foreach(key package name clap_path bundle_id version subtype manufacturer vendor)
        string(JSON ${key} GET "${plugin}" ${key})
    endforeach()
    if(NOT DEFINED AU_BUILD_CONFIG)
        message(FATAL_ERROR "AU_BUILD_CONFIG is required")
    endif()
    if(NOT AU_BUILD_CONFIG MATCHES "^(Debug|Release)$")
        message(FATAL_ERROR "AU_BUILD_CONFIG must be Debug or Release, got '${AU_BUILD_CONFIG}'")
    endif()
    set(output "${CMAKE_BINARY_DIR}/products/${AU_BUILD_CONFIG}")
    string(TOUPPER "${AU_BUILD_CONFIG}" output_config)
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/products/Release" "${CMAKE_BINARY_DIR}/products/Debug")
    set(target "${package}_${AU_FORMAT}")
    if(AU_FORMAT STREQUAL "auv2")
        add_library(${target} MODULE)
        target_add_auv2_wrapper(TARGET ${target} OUTPUT_NAME "${name}"
            BUNDLE_IDENTIFIER "${bundle_id}.auv2" BUNDLE_VERSION "${version}"
            MANUFACTURER_NAME "${vendor}" MANUFACTURER_CODE "${manufacturer}"
            SUBTYPE_CODE "${subtype}" INSTRUMENT_TYPE aufx
            MACOS_EMBEDDED_CLAP_LOCATION "${clap_path}")
        # v0.16.0 adds .component in target properties but not its Info.plist.
        set_target_properties(${target} PROPERTIES
            XCODE_ATTRIBUTE_PRODUCT_BUNDLE_IDENTIFIER "${bundle_id}.auv2"
            MACOSX_BUNDLE_GUI_IDENTIFIER "${bundle_id}.auv2")
        set(helper "${target}-build-helper")
    else()
        # An executable with the Xcode app-extension product type, not a MODULE.
        add_executable(${target})
        target_add_auv3_wrapper(TARGET ${target} OUTPUT_NAME "${name}"
            BUNDLE_IDENTIFIER "${bundle_id}.auv3.extension" BUNDLE_VERSION "${version}"
            MANUFACTURER_NAME "${vendor}" MANUFACTURER_CODE "${manufacturer}"
            SUBTYPE_CODE "${subtype}" INSTRUMENT_TYPE aufx
            MACOS_EMBEDDED_CLAP_LOCATION "${clap_path}")
        set(helper "${target}-auv3-build-helper")
        add_executable(${package}_app)
        target_add_auv3_standalone_wrapper(TARGET ${package}_app
            OUTPUT_NAME "${name} AUv3" BUNDLE_IDENTIFIER "${bundle_id}.auv3"
            BUNDLE_VERSION "${version}" AUV3_TARGET ${target}
            AU_TYPE aufx AU_SUBTYPE "${subtype}" AU_MANUFACTURER "${manufacturer}")
        # Upstream's host template hard-codes 1.0/1. Override only bundle metadata.
        set_target_properties(${package}_app PROPERTIES
            MACOSX_BUNDLE_INFO_PLIST "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../Host-Info.plist.in"
            MACOSX_BUNDLE_BUNDLE_VERSION "${version}"
            MACOSX_BUNDLE_SHORT_VERSION_STRING "${version}"
            RUNTIME_OUTPUT_DIRECTORY "${output}"
            RUNTIME_OUTPUT_DIRECTORY_${output_config} "${output}")
    endif()
    # clap-wrapper's embedded-CLAP PRE_BUILD command reads the generic
    # LIBRARY_OUTPUT_DIRECTORY property as its working directory. Keep that
    # property concrete (no nested $<CONFIG> expression), while also setting
    # the config-specific properties so Xcode does not append Release/Debug a
    # second time.
    set_target_properties(${target} PROPERTIES
        LIBRARY_OUTPUT_DIRECTORY "${output}"
        RUNTIME_OUTPUT_DIRECTORY "${output}"
        LIBRARY_OUTPUT_DIRECTORY_${output_config} "${output}"
        RUNTIME_OUTPUT_DIRECTORY_${output_config} "${output}")
    # Xcode does not honor LINK_DEPENDS. Change a helper-only translation unit
    # when CLAP bytes/metadata change so the helper relinks and its descriptor
    # generation POST_BUILD runs again. This never enters the shipped plugin.
    file(SHA256 "${clap_path}/Contents/MacOS/${name}" clap_hash)
    file(SHA256 "${AU_MANIFEST}" manifest_hash)
    set(stamp "${CMAKE_CURRENT_BINARY_DIR}/${target}-input-stamp.cpp")
    file(CONFIGURE OUTPUT "${stamp}" CONTENT
        "// CLAP @clap_hash@; manifest @manifest_hash@\nvoid @target@_input_stamp() {}\n" @ONLY)
    target_sources(${helper} PRIVATE "${stamp}")
endfunction()
