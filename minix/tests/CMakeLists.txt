# minix/tests/CMakeLists.txt
# Test program build
#
# Replicates minix/tests/Makefile and minix/tests/Makefile.inc

# Only build tests when testing framework is enabled
if(NOT MKATF)
    return()
endif()

# blocktest
add_subdirectory(blocktest)

# ddekit tests
add_subdirectory(ddekit)

# ds tests
add_subdirectory(ds)

# fbdtest
add_subdirectory(fbdtest)

# rmibtest
add_subdirectory(rmibtest)

# safecopy
add_subdirectory(safecopy)

# ============================================================================
# CTest integration
# ============================================================================

# (Testing is enabled in the root CMakeLists.txt)

# Add ATF/KYUA test discovery
foreach(TEST_SUBDIR blocktest ddekit ds fbdtest rmibtest safecopy)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/${TEST_SUBDIR}/Makefile")
        add_test(
            NAME ${TEST_SUBDIR}-check
            COMMAND ${CMAKE_COMMAND} -E env
                TESTDIR=${CMAKE_CURRENT_SOURCE_DIR}/${TEST_SUBDIR}
                ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_SUBDIR}/test.sh
            WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_SUBDIR}
        )
        set_tests_properties(${TEST_SUBDIR}-check PROPERTIES
            TIMEOUT 120
        )
    endif()
endforeach()
