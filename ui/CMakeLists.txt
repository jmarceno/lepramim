cmake_minimum_required(VERSION 3.21)

project(lexaloud-ui CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Widgets Network DBus Svg Test)

option(WARNINGS_AS_ERRORS "Treat warnings as errors" OFF)

if(WARNINGS_AS_ERRORS)
    add_compile_options(-Wall -Wextra -Wpedantic -Werror -Wconversion -Wsign-conversion)
else()
    add_compile_options(-Wall -Wextra -Wpedantic)
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Release")
    add_compile_options(-fvisibility=hidden -fvisibility-inlines-hidden)
endif()

set(UI_HEADERS
    include/api_client.hpp
    include/tray.hpp
    include/control_window.hpp
    include/onboarding.hpp
    include/overlay.hpp
)

set(UI_SOURCES
    src/api_client.cpp
    src/tray.cpp
    src/control_window.cpp
    src/onboarding.cpp
    src/overlay.cpp
    src/main.cpp
)

add_executable(lexaloud-ui ${UI_SOURCES} ${UI_HEADERS})

target_include_directories(lexaloud-ui PRIVATE include)

target_link_libraries(lexaloud-ui PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Network
    Qt6::DBus
    Qt6::Svg
)

enable_testing()

add_executable(test_api_client
    tests/test_api_client.cpp
    src/api_client.cpp
    include/api_client.hpp
)
target_include_directories(test_api_client PRIVATE include)
target_link_libraries(test_api_client PRIVATE Qt6::Core Qt6::Network Qt6::Test)
add_test(NAME test_api_client COMMAND test_api_client)

add_executable(test_control_window
    tests/test_control_window.cpp
    src/control_window.cpp
    src/api_client.cpp
    include/control_window.hpp
    include/api_client.hpp
)
target_include_directories(test_control_window PRIVATE include)
target_link_libraries(test_control_window PRIVATE Qt6::Core Qt6::Gui Qt6::Widgets Qt6::Network Qt6::Test)
add_test(NAME test_control_window COMMAND test_control_window)

add_executable(test_tray_state
    tests/test_tray_state.cpp
    src/tray.cpp
    src/api_client.cpp
    src/control_window.cpp
    include/tray.hpp
    include/api_client.hpp
    include/control_window.hpp
)
target_include_directories(test_tray_state PRIVATE include)
target_link_libraries(test_tray_state PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Network
    Qt6::DBus
    Qt6::Svg
    Qt6::Test
)
add_test(NAME test_tray_state COMMAND test_tray_state)
