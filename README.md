# Project m1: Multi-platform screen-grabbing utility

Using the Rust programming language, create a screen grabbing utility capable of acquiring what is currently shown in a display, post-process it and make it available in one or more formats.

The application should fulfill the following requirements:

- [x] **Platform Support**: The utility should be compatible with multiple desktop operating systems, including Windows, macOS, and Linux.
- [ ] **User Interface (UI)**: The utility should have an intuitive and user-friendly interface that allows users to easily navigate through the application's features.
- [ ] **Selection Options**: The utility should allow the user to restrict the grabbed image to a custom area selected with a click and drag motion. The selected area may be further adjusted with subsequent interactions.
- [ ] **Hotkey Support**: The utility should support customizable hotkeys for quick screen grabbing. Users should be able to set up their preferred shortcut keys.
- [x] **Output Format**: The utility should support multiple output formats including .png, .jpg, .gif. It should also support copying the screen grab to the clipboard.

As a bonus, the application may also provide the following features:

- [ ] **Annotation Tools**: The utility should have built-in annotation tools like shapes, arrows, text, and a color picker for highlighting or redacting parts of the screen grab.
- [x] **Delay Timer**: The utility should support a delay timer function, allowing users to set up a screen grab after a specified delay.
- [x] **Save Options**: The utility should allow users to specify the default save location for screen grabs. It should also support automatic saving with predefined naming conventions.
- [x] **Multi-monitor Support**: The utility should be able to recognize and handle multiple monitors independently, allowing users to grab screens from any of the connected displays.
