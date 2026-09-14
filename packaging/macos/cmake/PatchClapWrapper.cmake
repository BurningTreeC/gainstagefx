# Compatibility fix for clap-wrapper v0.16.0, commit
# 1cca996e96f29ab2be7ae9f8cfe532bbc92e1dd6. No DSP or plugin GUI changes.
function(patch_clap_wrapper_auv3 source_dir)
    set(source "${source_dir}/src/detail/standalone/macos/auv3/AUv3HostAppDelegate.mm")
    set(before "004dec3196e434a500c7fbd3e35bddbc67fda1df63873d027a0c18e7e1d03cc5")
    set(after "c505905f968a872a9620a3a7d744ac39658915915bd01bffcb57b8b85039569e")
    file(SHA256 "${source}" actual)
    if(actual STREQUAL after)
        return() # Reconfiguration of an already patched FetchContent checkout.
    endif()
    if(NOT actual STREQUAL before)
        message(FATAL_ERROR "Unexpected clap-wrapper AUv3 host source; review the compatibility patch before changing the dependency pin")
    endif()
    find_package(Git REQUIRED)
    execute_process(
        COMMAND "${GIT_EXECUTABLE}" apply --whitespace=error
            "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../patches/auv3-switch-scope.patch"
        WORKING_DIRECTORY "${source_dir}"
        RESULT_VARIABLE status OUTPUT_VARIABLE output ERROR_VARIABLE error)
    if(NOT status EQUAL 0)
        message(FATAL_ERROR "Cannot apply clap-wrapper AUv3 compatibility patch: ${output}${error}")
    endif()
    file(SHA256 "${source}" actual)
    if(NOT actual STREQUAL after)
        message(FATAL_ERROR "Patched clap-wrapper source hash mismatch")
    endif()
endfunction()
