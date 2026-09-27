/* @ds-bundle: {"format":4,"namespace":"Echo","components":[{"name":"Icon"},{"name":"FileIcon"},{"name":"Button"},{"name":"IconButton"},{"name":"SegmentedControl"},{"name":"Switch"},{"name":"Checkbox"},{"name":"Kbd"},{"name":"TextField"},{"name":"SearchField"},{"name":"PathBar"},{"name":"TabStrip"},{"name":"Toolbar"},{"name":"Sidebar"},{"name":"DriveItem"},{"name":"FileList"},{"name":"FileRow"},{"name":"FileGrid"},{"name":"StatusBar"},{"name":"EmptyState"},{"name":"Skeleton"},{"name":"DriveCard"},{"name":"UsageBar"},{"name":"StatePill"},{"name":"Banner"},{"name":"Toast"},{"name":"TransferToast"},{"name":"Spinner"},{"name":"Tooltip"},{"name":"Dialog"},{"name":"ConflictDialog"},{"name":"ContextMenu"},{"name":"CommandPalette"},{"name":"PreviewPane"},{"name":"SearchScope"},{"name":"SearchResults"},{"name":"SettingsGroup"},{"name":"SettingRow"},{"name":"PathListEditor"},{"name":"NameChips"},{"name":"IndexStatus"},{"name":"CommandList"},{"name":"SettingsNav"},{"name":"SettingsPage"},{"name":"AppWindow"},{"name":"DualPane"},{"name":"MotionSpec"}]} */
(function(){
var ICONS = {"glyphs":{"arrow-left":"<path d=\"M19 12H5M11 6l-6 6 6 6\"/>","arrow-right":"<path d=\"M5 12h14M13 6l6 6-6 6\"/>","arrow-up":"<path d=\"M12 19V5M6 11l6-6 6 6\"/>","arrow-down":"<path d=\"M12 5v14M6 13l6 6 6-6\"/>","chevron-left":"<path d=\"M15 6l-6 6 6 6\"/>","chevron-right":"<path d=\"M9 6l6 6-6 6\"/>","chevron-up":"<path d=\"M6 15l6-6 6 6\"/>","chevron-down":"<path d=\"M6 9l6 6 6-6\"/>","home":"<path d=\"M4 10.5L12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z\"/>","history":"<path d=\"M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5M3.5 4v4.5H8M12 8v4.5l3 2\"/>","grid":"<rect x=\"4\" y=\"4\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"13.5\" y=\"4\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"4\" y=\"13.5\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"13.5\" y=\"13.5\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/>","list":"<path d=\"M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01\"/>","columns":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M12 4v16\"/>","sidebar":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M9 4v16\"/>","sort":"<path d=\"M7 4v16M3.5 7.5L7 4l3.5 3.5M17 20V4M13.5 16.5L17 20l3.5-3.5\"/>","filter":"<path d=\"M4 5h16l-6 7.5V19l-4-2v-4.5z\"/>","sliders":"<path d=\"M4 7h9M17 7h3M4 17h3M11 17h9\"/><circle cx=\"15\" cy=\"7\" r=\"2\"/><circle cx=\"9\" cy=\"17\" r=\"2\"/>","more-vertical":"<path d=\"M12 5h.01M12 12h.01M12 19h.01\" stroke-width=\"3\"/>","more-horizontal":"<path d=\"M5 12h.01M12 12h.01M19 12h.01\" stroke-width=\"3\"/>","search":"<circle cx=\"11\" cy=\"11\" r=\"6.5\"/><path d=\"M16 16l4.5 4.5\"/>","eye":"<path d=\"M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/>","eye-off":"<path d=\"M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/><path d=\"M4 4l16 16\"/>","copy":"<rect x=\"8\" y=\"8\" width=\"12\" height=\"12\" rx=\"2\"/><path d=\"M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2\"/>","cut":"<circle cx=\"6.5\" cy=\"17.5\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"17.5\" r=\"2.5\"/><path d=\"M8.3 15.7L18 4M15.7 15.7L6 4\"/>","paste":"<rect x=\"5\" y=\"5\" width=\"14\" height=\"16\" rx=\"2\"/><path d=\"M9 5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1M9 11h6M9 15h4\"/>","rename":"<path d=\"M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3\"/>","trash":"<path d=\"M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2\"/>","undo":"<path d=\"M9 14l-5-5 5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11\"/>","redo":"<path d=\"M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13\"/>","plus":"<path d=\"M12 5v14M5 12h14\"/>","minus":"<path d=\"M5 12h14\"/>","close":"<path d=\"M6 6l12 12M18 6L6 18\"/>","check":"<path d=\"M5 12.5l4.5 4.5L19 7.5\"/>","folder":"<path d=\"M3.5 7a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z\"/>","folder-plus":"<path d=\"M3.5 7a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z\"/><path d=\"M12 10.5v5M9.5 13h5\"/>","file":"<path d=\"M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM14 3.5v5h5\"/>","file-plus":"<path d=\"M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM14 3.5v5h5\"/><path d=\"M12 11.5v6M9 14.5h6\"/>","download":"<path d=\"M12 4v11M7 10l5 5 5-5M5 20h14\"/>","upload":"<path d=\"M12 16V5M7 10l5-5 5 5M5 20h14\"/>","move":"<path d=\"M14 6l5 5-5 5M19 11h-8a6 6 0 0 0-6 6\"/>","swap":"<path d=\"M4 8h15M15 4l4 4-4 4M20 16H5M9 12l-4 4 4 4\"/>","merge":"<path d=\"M12 4v16M3 12h6M6.5 9.5L9 12l-2.5 2.5M21 12h-6M17.5 9.5L15 12l2.5 2.5\"/>","compress":"<path d=\"M4 4l5 5M9 5v4H5M20 4l-5 5M15 5v4h4M4 20l5-5M9 19v-4H5M20 20l-5-5M15 19v-4h4\"/>","external":"<path d=\"M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4\"/>","link":"<path d=\"M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1\"/>","share":"<circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/>","send":"<path d=\"M21 3L10 14M21 3l-6.5 18-4.5-7-7-4.5z\"/>","sync":"<path d=\"M20 11a8 8 0 0 0-14.3-4.3L4 8.5M4 4v4.5h4.5M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8M20 20v-4.5h-4.5\"/>","refresh":"<path d=\"M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5\"/>","cloud":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5z\"/>","cloud-upload":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15\"/><path d=\"M12 20v-7M9.5 15.5L12 13l2.5 2.5M9 18H7\"/>","cloud-download":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15M9 18H7\"/><path d=\"M12 12v8M9.5 17.5L12 20l2.5-2.5\"/>","star":"<path d=\"M12 3.5l2.6 5.3 5.9.9-4.25 4.1 1 5.8L12 16.9l-5.25 2.7 1-5.8L3.5 9.7l5.9-.9z\"/>","tag":"<path d=\"M3.5 12.3V4.5a1 1 0 0 1 1-1h7.8a2 2 0 0 1 1.4.6l7.2 7.2a2 2 0 0 1 0 2.8l-6.6 6.6a2 2 0 0 1-2.8 0l-7.2-7.2a2 2 0 0 1-.6-1.4z\"/><path d=\"M8 8h.01\" stroke-width=\"3\"/>","pin":"<path d=\"M9 4h6l-1 5 3 3v2H7v-2l3-3zM12 14v6\"/>","info":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 11v5M12 7.5h.01\"/>","alert":"<path d=\"M12 3.5l9.5 16.5h-19zM12 10v4M12 17h.01\"/>","error":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7.5v5M12 16h.01\"/>","clock":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7v5l3 2\"/>","lock":"<rect x=\"5\" y=\"10.5\" width=\"14\" height=\"10\" rx=\"2\"/><path d=\"M8 10.5V7.5a4 4 0 0 1 8 0v3M12 14.5v2\"/>","unlock":"<rect x=\"5\" y=\"10.5\" width=\"14\" height=\"10\" rx=\"2\"/><path d=\"M8 10.5V7.5a4 4 0 0 1 7.6-1.7M12 14.5v2\"/>","key":"<circle cx=\"8\" cy=\"15\" r=\"4.5\"/><path d=\"M11.2 11.8L20 3M16.5 6.5l2.5 2.5M14 9l2 2\"/>","shield":"<path d=\"M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z\"/><path d=\"M8.5 12l2.5 2.5 4.5-4.5\"/>","heart":"<path d=\"M12 20s-7.5-4.6-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.4-7.5 10-7.5 10z\"/>","user":"<circle cx=\"12\" cy=\"8\" r=\"3.5\"/><path d=\"M5 20a7 7 0 0 1 14 0\"/>","users":"<circle cx=\"9\" cy=\"8.5\" r=\"3\"/><path d=\"M3.5 19.5a5.5 5.5 0 0 1 11 0M15.5 5.8a3 3 0 0 1 0 5.4M17 14.2a5.5 5.5 0 0 1 3.5 5.3\"/>","settings":"<path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/>","terminal":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M7 9l3 3-3 3M13 15h4\"/>","command":"<rect x=\"3\" y=\"5\" width=\"18\" height=\"14\" rx=\"2\"/><path d=\"M7 10l2.5 2L7 14M12 14h5\"/>","keyboard":"<rect x=\"2.5\" y=\"6\" width=\"19\" height=\"12\" rx=\"2\"/><path d=\"M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M8 14h8\"/>","drive":"<rect x=\"3\" y=\"6\" width=\"18\" height=\"12\" rx=\"2\"/><path d=\"M3 13h18M16.5 15.5h.01M13.5 15.5h.01\"/>","usb":"<path d=\"M8 10h8v9a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2zM9.5 10V3.5h5V10M11 6.5h.01M13 6.5h.01\"/>","sd-card":"<path d=\"M8 3h8.5L19 5.5V20a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6zM9 7v3M12 7v3M15 7v3\"/>","phone":"<rect x=\"6.5\" y=\"2.5\" width=\"11\" height=\"19\" rx=\"2.5\"/><path d=\"M10.5 18.5h3\"/>","network":"<rect x=\"9\" y=\"3\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"3\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"15\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/><path d=\"M12 8v4M6 16v-2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2\"/>","server":"<rect x=\"3.5\" y=\"4\" width=\"17\" height=\"7\" rx=\"1.5\"/><rect x=\"3.5\" y=\"13\" width=\"17\" height=\"7\" rx=\"1.5\"/><path d=\"M7 7.5h.01M7 16.5h.01\"/>","image":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><circle cx=\"8.5\" cy=\"9.5\" r=\"1.5\"/><path d=\"M21 16l-5-5-9 9\"/>","music":"<path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/>","video":"<rect x=\"3\" y=\"5\" width=\"13\" height=\"14\" rx=\"2\"/><path d=\"M16 10l5-3v10l-5-3\"/>","play":"<path d=\"M7 4.5v15l12-7.5z\"/>","pause":"<path d=\"M9 5v14M15 5v14\"/>","cancel":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M9 9l6 6M15 9l-6 6\"/>","crop":"<path d=\"M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14\"/>","rotate":"<path d=\"M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5\"/>","archive":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"5\" rx=\"1\"/><path d=\"M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4\"/>","code":"<path d=\"M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4.5l-3 15\"/>"},"color":{"folder":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/>","folder-open":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M9.3 18h35.2a2 2 0 0 1 1.9 2.7l-5.5 16.5A4 4 0 0 1 37.1 40H6a2 2 0 0 1-1.9-2.7l5.3-17.4A2 2 0 0 1 9.3 18z\"/>","folder-plus":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M5 12h14\"/></g>","folder-minus":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12h14\"/></g>","folder-up":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 19V5M6 11l6-6 6 6\"/></g>","folder-download":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 4v11M7 10l5 5 5-5M5 20h14\"/></g>","folder-link":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1\"/></g>","folder-heart":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"var(--icon-folder-glyph)\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 20s-7.5-4.6-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.4-7.5 10-7.5 10z\"/></g>","folder-lock":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><rect x=\"18\" y=\"26\" width=\"12\" height=\"9.5\" rx=\"2\" fill=\"var(--icon-folder-glyph)\"/><path d=\"M20.5 26v-2.5a3.5 3.5 0 0 1 7 0V26\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"2.4\"/>","folder-clock":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7v5l3 2\"/></g>","folder-move":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M14 6l5 5-5 5M19 11h-8a6 6 0 0 0-6 6\"/></g>","folder-share":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/></g>","folder-users":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"9\" cy=\"8.5\" r=\"3\"/><path d=\"M3.5 19.5a5.5 5.5 0 0 1 11 0M15.5 5.8a3 3 0 0 1 0 5.4M17 14.2a5.5 5.5 0 0 1 3.5 5.3\"/></g>","folder-user":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"8\" r=\"3.5\"/><path d=\"M5 20a7 7 0 0 1 14 0\"/></g>","folder-settings":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","folder-search":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"11\" cy=\"11\" r=\"6.5\"/><path d=\"M16 16l4.5 4.5\"/></g>","folder-image":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><circle cx=\"8.5\" cy=\"9.5\" r=\"1.5\"/><path d=\"M21 16l-5-5-9 9\"/></g>","folder-music":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/></g>","folder-video":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path d=\"M21 23.5v11l9-5.5z\" fill=\"var(--icon-folder-glyph)\"/>","home":"<path fill=\"var(--icon-blue)\" d=\"M24 5.5l18 15.2V40a3 3 0 0 1-3 3H29.5V31h-11v12H9a3 3 0 0 1-3-3V20.7z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M24 5.5l18 15.2v3.1L24 8.6 6 23.8v-3.1z\"/>","trash":"<rect x=\"15\" y=\"4\" width=\"18\" height=\"6\" rx=\"2\" fill=\"var(--icon-slate)\"/><rect x=\"7\" y=\"8\" width=\"34\" height=\"6\" rx=\"2\" fill=\"var(--icon-slate)\"/><path fill=\"var(--icon-paper-fold)\" d=\"M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z\"/><path d=\"M20 22v14M28 22v14\" stroke=\"var(--icon-slate)\" stroke-width=\"3\" stroke-linecap=\"round\"/>","trash-full":"<rect x=\"15\" y=\"4\" width=\"18\" height=\"6\" rx=\"2\" fill=\"var(--icon-red)\"/><rect x=\"7\" y=\"8\" width=\"34\" height=\"6\" rx=\"2\" fill=\"var(--icon-red)\"/><path fill=\"var(--icon-red)\" d=\"M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z\"/><path d=\"M20 22v14M28 22v14\" stroke=\"var(--icon-on)\" stroke-width=\"3\" stroke-linecap=\"round\" opacity=\"0.9\"/>","drive":"<path fill=\"var(--icon-slate)\" d=\"M11.5 9h25a3 3 0 0 1 2.9 2.3L43 27H5l3.6-15.7A3 3 0 0 1 11.5 9z\"/><rect x=\"5\" y=\"26\" width=\"38\" height=\"13\" rx=\"3\" fill=\"var(--icon-slate-deep)\"/><circle cx=\"36\" cy=\"32.5\" r=\"2\" fill=\"var(--icon-on)\"/><circle cx=\"30\" cy=\"32.5\" r=\"2\" fill=\"var(--icon-on)\" opacity=\"0.7\"/>","drive-external":"<rect x=\"5\" y=\"12\" width=\"38\" height=\"24\" rx=\"5\" fill=\"var(--icon-slate-deep)\"/><rect x=\"5\" y=\"12\" width=\"38\" height=\"12\" rx=\"5\" fill=\"var(--icon-slate)\"/><rect x=\"5\" y=\"20\" width=\"38\" height=\"4\" fill=\"var(--icon-slate)\"/><circle cx=\"36\" cy=\"30\" r=\"2\" fill=\"var(--icon-on)\"/>","server":"<rect x=\"6\" y=\"6\" width=\"36\" height=\"16\" rx=\"3\" fill=\"var(--icon-slate)\"/><rect x=\"6\" y=\"26\" width=\"36\" height=\"16\" rx=\"3\" fill=\"var(--icon-slate-deep)\"/><circle cx=\"34\" cy=\"14\" r=\"2\" fill=\"var(--icon-on)\"/><circle cx=\"34\" cy=\"34\" r=\"2\" fill=\"var(--icon-on)\"/><rect x=\"12\" y=\"12.7\" width=\"12\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\" opacity=\"0.6\"/>","usb":"<rect x=\"16\" y=\"4\" width=\"16\" height=\"14\" rx=\"2\" fill=\"var(--icon-paper-fold)\"/><rect x=\"20\" y=\"8\" width=\"3\" height=\"4\" rx=\"1\" fill=\"var(--icon-slate)\"/><rect x=\"25\" y=\"8\" width=\"3\" height=\"4\" rx=\"1\" fill=\"var(--icon-slate)\"/><rect x=\"12\" y=\"16\" width=\"24\" height=\"28\" rx=\"5\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"34\" width=\"8\" height=\"3\" rx=\"1.5\" fill=\"var(--icon-paper-fold)\"/>","sd-card":"<path fill=\"var(--icon-slate-deep)\" d=\"M15 4h19l6 6v30a4 4 0 0 1-4 4H12a4 4 0 0 1-4-4V11z\"/><rect x=\"16\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"21\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"26\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"31\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/>","phone":"<rect x=\"12\" y=\"3\" width=\"24\" height=\"42\" rx=\"6\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"7\" width=\"18\" height=\"31\" rx=\"2.5\" fill=\"var(--icon-on)\" opacity=\"0.92\"/><rect x=\"20\" y=\"40.2\" width=\"8\" height=\"2.2\" rx=\"1.1\" fill=\"var(--icon-on)\"/>","network":"<rect x=\"17\" y=\"4\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><rect x=\"5\" y=\"33\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><rect x=\"29\" y=\"33\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><path d=\"M24 15v8M12 33v-4a3 3 0 0 1 3-3h18a3 3 0 0 1 3 3v4\" fill=\"none\" stroke=\"var(--icon-blue-deep)\" stroke-width=\"3\" stroke-linecap=\"round\"/>","cloud":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/>","cloud-upload":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/><g transform=\"translate(16 17) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.50\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 19V5M6 11l6-6 6 6\"/></g>","cloud-download":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/><g transform=\"translate(16 18) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.50\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M6 13l6 6 6-6\"/></g>","file":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/>","file-text":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/>","file-doc":"<path fill=\"var(--icon-blue)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/>","file-sheet":"<path fill=\"var(--icon-green)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"21\" width=\"19\" height=\"15\" rx=\"1.5\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/><path d=\"M15 28.5h19M24.5 21v15\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/>","file-slides":"<path fill=\"var(--icon-orange)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"12\" rx=\"1.5\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/><rect x=\"18.5\" y=\"26.5\" width=\"12\" height=\"3\" rx=\"1\" fill=\"var(--icon-on)\"/>","file-pdf":"<path fill=\"var(--icon-red)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"19\" width=\"12\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"24\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"13\" y=\"30\" width=\"23\" height=\"9\" rx=\"2\" fill=\"var(--icon-on)\"/><path d=\"M16.5 37v-5h2a1.5 1.5 0 0 1 0 3h-2M22.5 37v-5h1.5a2.5 2.5 0 0 1 0 5zM29.5 37v-5h3M29.5 34.5h2.4\" fill=\"none\" stroke=\"var(--icon-red)\" stroke-width=\"1.3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>","file-code":"<path fill=\"var(--icon-purple)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(14.5 20) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"3.36\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4.5l-3 15\"/></g>","file-config":"<path fill=\"var(--icon-slate)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(15 20) scale(0.7500)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"3.47\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","file-font":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M14.5 34l5-14h3l5 14h-3.1l-1.1-3.3h-4.6L17.6 34zm5-5.9h3l-1.5-4.6z\"/><path fill=\"var(--icon-slate)\" d=\"M31.6 34.2c-2 0-3.3-1.2-3.3-3 0-2 1.5-3 4.3-3.1l1.9-.1v-.4c0-.9-.6-1.4-1.7-1.4-1 0-1.6.4-1.8 1.1h-2.6c.2-2.1 2-3.4 4.5-3.4 2.7 0 4.3 1.3 4.3 3.6V34h-2.6v-1.3c-.5.9-1.6 1.5-3 1.5zm.9-2.1c1.1 0 2-.7 2-1.7v-.6l-1.6.1c-1 .1-1.5.5-1.5 1.1s.4 1.1 1.1 1.1z\"/>","file-type":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M16 20h17v3.4h-6.8V36h-3.4V23.4H16z\"/>","file-lines":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"20\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"25\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"30\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"35\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/>","file-list":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"16.5\" cy=\"22.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"21\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><circle cx=\"16.5\" cy=\"27.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"26\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><circle cx=\"16.5\" cy=\"32.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"31\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/>","file-script":"<path fill=\"var(--icon-slate-deep)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(13 21) scale(0.5833)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.14\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 6l6 6-6 6\"/></g><rect x=\"25\" y=\"32\" width=\"9\" height=\"2.8\" rx=\"1.4\" fill=\"var(--icon-on)\"/>","file-image":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-green)\" opacity=\"0.10\"/><circle cx=\"19\" cy=\"23\" r=\"3\" fill=\"var(--icon-orange)\"/><path fill=\"var(--icon-green)\" d=\"M13 38l7.5-9 4.5 5 4-4.5 7 8.5z\"/>","file-video":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-red)\" opacity=\"0.10\"/><rect x=\"14\" y=\"21\" width=\"21\" height=\"15\" rx=\"2.5\" fill=\"var(--icon-red)\"/><path d=\"M22 24.8v7.4l6-3.7z\" fill=\"var(--icon-on)\"/>","file-audio":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-purple)\" opacity=\"0.10\"/><g transform=\"translate(14 19) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"3.36\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/></g>","file-vector":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-purple)\" opacity=\"0.10\"/><path fill=\"var(--icon-purple)\" d=\"M13 38l7.5-10 4.5 6 3-4 6 8z\"/><g transform=\"translate(23 17) scale(0.5833)\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"4.46\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3\"/></g>","file-archive":"<path fill=\"var(--icon-folder)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-folder-back)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"24\" y=\"6.0\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"9.4\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"12.8\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"16.2\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"19.6\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"23.0\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"26.4\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"29.8\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"20.5\" y=\"33\" width=\"7\" height=\"6\" rx=\"1.5\" fill=\"var(--icon-folder-glyph)\"/>","file-archive-alt":"<path fill=\"var(--icon-slate)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-slate-deep)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"24\" y=\"6.0\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"9.4\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"12.8\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"16.2\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"19.6\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"23.0\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"26.4\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"29.8\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"33.2\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"36.599999999999994\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"20.5\" y=\"39\" width=\"7\" height=\"3\" rx=\"1\" fill=\"var(--icon-slate-deep)\"/>","file-package":"<rect x=\"6\" y=\"7\" width=\"36\" height=\"11\" rx=\"3\" fill=\"var(--icon-red)\"/><rect x=\"6\" y=\"18\" width=\"36\" height=\"11\" rx=\"0\" fill=\"var(--icon-blue)\"/><rect x=\"6\" y=\"29\" width=\"36\" height=\"12\" rx=\"3\" fill=\"var(--icon-green)\"/><rect x=\"21\" y=\"7\" width=\"7\" height=\"34\" fill=\"var(--icon-folder)\"/><rect x=\"19.5\" y=\"20.5\" width=\"10\" height=\"7\" rx=\"1.5\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"36\" y=\"10\" width=\"2.4\" height=\"5\" rx=\"1\" fill=\"var(--icon-on)\" opacity=\"0.8\"/>","file-verified":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-blue)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><g transform=\"translate(30 30) scale(0.5000)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.20\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12.5l4.5 4.5L19 7.5\"/></g>","file-error":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-red)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><path d=\"M36 31v6\" stroke=\"var(--icon-on)\" stroke-width=\"2.8\" stroke-linecap=\"round\"/><circle cx=\"36\" cy=\"40.6\" r=\"1.6\" fill=\"var(--icon-on)\"/>","file-new":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-blue)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><g transform=\"translate(30 30) scale(0.5000)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.20\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M5 12h14\"/></g>","lock":"<path d=\"M16 21v-5a8 8 0 0 1 16 0v5\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"4.5\"/><rect x=\"10\" y=\"20\" width=\"28\" height=\"23\" rx=\"5\" fill=\"var(--icon-purple)\"/><circle cx=\"24\" cy=\"30\" r=\"3\" fill=\"var(--icon-on)\"/><rect x=\"22.6\" y=\"31\" width=\"2.8\" height=\"6\" rx=\"1.4\" fill=\"var(--icon-on)\"/>","shield":"<path fill=\"var(--icon-green)\" d=\"M24 4l16 6v11.5C40 31.5 33.3 39.6 24 44 14.7 39.6 8 31.5 8 21.5V10z\"/><g transform=\"translate(14 13) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.08\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12.5l4.5 4.5L19 7.5\"/></g>","key":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"8\" cy=\"15\" r=\"4.5\"/><path d=\"M11.2 11.8L20 3M16.5 6.5l2.5 2.5M14 9l2 2\"/></g>","eye":"<path fill=\"var(--icon-blue)\" d=\"M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z\"/><circle cx=\"24\" cy=\"24\" r=\"7.5\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"24\" r=\"4\" fill=\"var(--icon-blue-deep)\"/>","eye-off":"<path fill=\"var(--icon-paper-fold)\" d=\"M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z\"/><circle cx=\"24\" cy=\"24\" r=\"7.5\" fill=\"var(--icon-slate)\"/><path d=\"M9 9l30 30\" stroke=\"var(--icon-slate)\" stroke-width=\"4.5\" stroke-linecap=\"round\"/>","share":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-blue)\" stroke-width=\"2.51\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/></g>","send":"<path fill=\"var(--icon-blue)\" d=\"M43 5L4.5 20.5l14 6.5 6.5 14z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M43 5L18.5 27 25 41z\"/>","sync":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-green)\" stroke-width=\"2.51\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M20 11a8 8 0 0 0-14.3-4.3L4 8.5M4 4v4.5h4.5M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8M20 20v-4.5h-4.5\"/></g>","history":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5M3.5 4v4.5H8M12 8v4.5l3 2\"/></g>","star":"<path fill=\"var(--icon-orange)\" d=\"M24 4.5l5.8 11.8 13 1.9-9.4 9.2 2.2 12.9L24 34.2l-11.6 6.1 2.2-12.9-9.4-9.2 13-1.9z\"/>","tag":"<path fill=\"var(--icon-blue)\" d=\"M5 22.3V8a3 3 0 0 1 3-3h14.3a4 4 0 0 1 2.8 1.2l16.2 16.2a4 4 0 0 1 0 5.6L27.6 41.7a4 4 0 0 1-5.6 0L6.2 25.1A4 4 0 0 1 5 22.3z\"/><circle cx=\"14.5\" cy=\"14.5\" r=\"3.5\" fill=\"var(--icon-on)\"/>","info":"<circle cx=\"24\" cy=\"24\" r=\"20\" fill=\"var(--icon-blue)\"/><rect x=\"21.5\" y=\"21\" width=\"5\" height=\"14\" rx=\"2.5\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"14.5\" r=\"3\" fill=\"var(--icon-on)\"/>","settings":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","sliders":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M4 7h9M17 7h3M4 17h3M11 17h9\"/><circle cx=\"15\" cy=\"7\" r=\"2\"/><circle cx=\"9\" cy=\"17\" r=\"2\"/></g>","terminal":"<rect x=\"4\" y=\"6\" width=\"40\" height=\"36\" rx=\"6\" fill=\"var(--icon-slate-deep)\"/><g transform=\"translate(9 14) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.08\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 6l6 6-6 6\"/></g><rect x=\"25\" y=\"29\" width=\"11\" height=\"3.4\" rx=\"1.7\" fill=\"var(--icon-on)\"/>","alert":"<path fill=\"var(--icon-orange)\" d=\"M21.4 6.5a3 3 0 0 1 5.2 0l17 29.5a3 3 0 0 1-2.6 4.5H7a3 3 0 0 1-2.6-4.5z\"/><rect x=\"21.6\" y=\"16\" width=\"4.8\" height=\"13\" rx=\"2.4\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"34\" r=\"2.6\" fill=\"var(--icon-on)\"/>"}};
(function () {
  var React = window.React;
  var h = React.createElement;
  var useState = React.useState, useEffect = React.useEffect, useRef = React.useRef;

  function cx() {
    var out = [];
    for (var i = 0; i < arguments.length; i++) if (arguments[i]) out.push(arguments[i]);
    return out.join(" ");
  }
  function rest(props, omit) {
    var o = {};
    for (var k in props) if (omit.indexOf(k) < 0) o[k] = props[k];
    return o;
  }

  // ------------------------------------------------------------ icons
  function Icon(p) {
    var size = p.size || 16;
    return h("svg", {
      className: cx("ef-icon", p.className), width: size, height: size, viewBox: "0 0 24 24",
      fill: "none", stroke: "currentColor", strokeWidth: p.strokeWidth || 2, strokeLinecap: "round",
      strokeLinejoin: "round", "aria-hidden": p.label ? undefined : "true", role: p.label ? "img" : undefined,
      "aria-label": p.label, dangerouslySetInnerHTML: { __html: ICONS.glyphs[p.name] || "" }
    });
  }

  var EXT = {
    jpg: "file-image", jpeg: "file-image", png: "file-image", webp: "file-image", heic: "file-image", gif: "file-image", avif: "file-image",
    mp4: "file-video", mkv: "file-video", mov: "file-video", webm: "file-video",
    mp3: "file-audio", flac: "file-audio", ogg: "file-audio", wav: "file-audio", m4a: "file-audio",
    pdf: "file-pdf", doc: "file-doc", docx: "file-doc", odt: "file-doc", md: "file-text", txt: "file-lines",
    xls: "file-sheet", xlsx: "file-sheet", csv: "file-sheet", ods: "file-sheet", ppt: "file-slides", pptx: "file-slides", odp: "file-slides",
    rs: "file-code", js: "file-code", ts: "file-code", py: "file-code", c: "file-code", html: "file-code", json: "file-code",
    toml: "file-config", ini: "file-config", conf: "file-config", yaml: "file-config", yml: "file-config",
    sh: "file-script", ps1: "file-script", bat: "file-script", exe: "file-script",
    zip: "file-archive", "7z": "file-archive", tar: "file-archive-alt", gz: "file-archive-alt", zst: "file-archive-alt", rar: "file-package",
    ttf: "file-font", otf: "file-font", woff2: "file-font", svg: "file-vector", lnk: "file", iso: "drive"
  };
  function iconFor(name, kind) {
    if (kind === "folder") return "folder";
    var m = /\.([^.]+)$/.exec(name || "");
    return (m && EXT[m[1].toLowerCase()]) || "file";
  }

  function FileIcon(p) {
    var size = p.size || 18;
    var name = p.name || "file";
    var badge = null;
    if (p.badge === "link") badge = h("span", { className: "ef-badge", title: "Link" }, h(Icon, { name: "link", size: 9, strokeWidth: 3 }));
    if (p.badge === "cloud") badge = h("span", { className: "ef-badge ef-badge-cloud", title: "Cloud-only placeholder" }, h(Icon, { name: "cloud", size: 9, strokeWidth: 3 }));
    if (p.badge === "lock") badge = h("span", { className: "ef-badge", title: "Encrypted (EFS)" }, h(Icon, { name: "lock", size: 9, strokeWidth: 3 }));
    if (p.badge === "broken") badge = h("span", { className: "ef-badge ef-badge-broken", title: "Broken link" }, h(Icon, { name: "close", size: 9, strokeWidth: 3.5 }));
    if (p.badge === "ads") badge = h("span", { className: "ef-badge ef-badge-ads", title: "Has alternate data streams" }, "+ADS");
    return h("span", { className: cx("ef-ficon", p.className), style: { width: size, height: size }, "aria-hidden": "true" },
      h("svg", { viewBox: "0 0 48 48", dangerouslySetInnerHTML: { __html: ICONS.color[name] || ICONS.color.file } }),
      size >= 18 ? badge : null);
  }

  // ------------------------------------------------------------ actions
  function Kbd(p) {
    var keys = p.keys || [];
    return h("span", { className: "ef-kbd", "aria-label": keys.join("+") }, keys.map(function (k, i) { return h("kbd", { key: i }, k); }));
  }

  function Button(p) {
    var o = rest(p, ["variant", "size", "icon", "kbd", "className", "children"]);
    o.className = cx("ef-btn", p.variant && "ef-btn-" + p.variant, p.size === "sm" && "ef-btn-sm", p.className);
    o.type = o.type || "button";
    return h("button", o, p.icon ? h(Icon, { name: p.icon, size: 14 }) : null, p.children, p.kbd ? h(Kbd, { keys: p.kbd }) : null);
  }

  function IconButton(p) {
    var o = rest(p, ["icon", "label", "pressed", "className"]);
    o.className = cx("ef-ibtn", p.className);
    o.type = "button";
    o["aria-label"] = p.label;
    o.title = p.label;
    if (p.pressed !== undefined) o["aria-pressed"] = String(!!p.pressed);
    return h("button", o, h(Icon, { name: p.icon, size: 16 }));
  }

  function SegmentedControl(p) {
    var st = useState(p.value !== undefined ? p.value : (p.options[0] || {}).value);
    var v = p.value !== undefined ? p.value : st[0];
    return h("div", { className: "ef-seg", role: "group", "aria-label": p.label },
      p.options.map(function (o) {
        return h("button", {
          key: o.value, type: "button", "aria-pressed": String(o.value === v), title: o.title || o.label,
          "aria-label": o.label || o.title,
          onClick: function () { st[1](o.value); p.onChange && p.onChange(o.value); }
        }, o.icon ? h(Icon, { name: o.icon, size: 14 }) : null, o.text || null);
      }));
  }

  function Switch(p) {
    var st = useState(!!p.defaultChecked);
    var on = p.checked !== undefined ? p.checked : st[0];
    var sw = h("button", {
      type: "button", role: "switch", "aria-checked": String(on), className: "ef-switch", id: p.id,
      "aria-label": p.children ? undefined : p.label,
      onClick: function () { st[1](!on); p.onChange && p.onChange(!on); }
    });
    if (!p.children) return sw;
    return h("label", { className: "ef-toggle-row" }, h("span", null, p.children), sw);
  }

  function Checkbox(p) {
    var st = useState(!!p.defaultChecked);
    var val = p.checked !== undefined ? p.checked : st[0];
    var aria = val === "mixed" ? "mixed" : String(!!val);
    return h("label", { className: "ef-label" },
      h("button", {
        type: "button", role: "checkbox", "aria-checked": aria, className: "ef-check", id: p.id,
        onClick: function () { var n = val === true ? false : true; st[1](n); p.onChange && p.onChange(n); }
      }, h(Icon, { name: aria === "mixed" ? "minus" : "check", size: 12, strokeWidth: 3 })),
      p.children ? h("span", null, p.children) : null);
  }

  // ------------------------------------------------------------ inputs
  function TextField(p) {
    var o = rest(p, ["label", "icon", "error", "className", "trailing", "id"]);
    return h("div", { className: p.className },
      p.label ? h("label", { className: "ef-field-label", htmlFor: p.id }, p.label) : null,
      h("div", { className: cx("ef-field", p.error && "ef-field-invalid") },
        p.icon ? h(Icon, { name: p.icon, size: 14 }) : null,
        h("input", Object.assign({ id: p.id, "aria-invalid": p.error ? "true" : undefined }, o)),
        p.trailing || null),
      p.error ? h("div", { className: "ef-field-msg", role: "alert" }, p.error) : null);
  }

  function SearchField(p) {
    return h("div", { className: cx("ef-field ef-search", p.className), role: "search" },
      h(Icon, { name: "search", size: 14 }),
      h("input", { id: p.id, type: "search", placeholder: p.placeholder || "Search", defaultValue: p.defaultValue, "aria-label": "Search" }),
      p.scope ? h("span", { className: "ef-chip" }, p.scope) : h(Kbd, { keys: ["Ctrl", "F"] }));
  }

  function PathBar(p) {
    var st = useState(!!p.editing);
    var editing = st[0];
    var segs = p.segments || [];
    if (editing) {
      return h("div", { className: "ef-field ef-path-edit" },
        h(Icon, { name: "folder", size: 14 }),
        h("input", { id: p.id, autoFocus: true, defaultValue: p.path || "/" + segs.map(function (s) { return s.label; }).join("/"), "aria-label": "Location", onBlur: function () { st[1](false); } }),
        h(Kbd, { keys: ["Enter"] }));
    }
    var kids = [];
    segs.forEach(function (s, i) {
      if (i > 0) kids.push(h(Icon, { key: "sep" + i, name: "chevron-right", size: 12, className: "ef-path-sep" }));
      kids.push(h("button", {
        key: i, type: "button", className: cx("ef-path-seg", i === segs.length - 1 && p.animateLast && "ef-path-new"),
        "aria-current": i === segs.length - 1 ? "location" : undefined
      }, s.icon ? h(Icon, { name: s.icon, size: 14 }) : null, s.label));
    });
    return h("nav", { className: "ef-path", "aria-label": "Location", onDoubleClick: function () { st[1](true); }, title: "Double-click or Ctrl+L to type a path" }, kids);
  }

  // ------------------------------------------------------------ navigation
  function TabStrip(p) {
    var st = useState(p.active || 0);
    return h("div", { className: "ef-tabs", role: "tablist" },
      (p.tabs || []).map(function (t, i) {
        return h("button", { key: i, type: "button", role: "tab", className: "ef-tab", "aria-selected": String(i === st[0]), onClick: function () { st[1](i); } },
          h(Icon, { name: t.icon || "folder", size: 14 }),
          h("span", { className: "ef-tab-name" }, t.label),
          h("span", { className: "ef-tab-close", "aria-label": "Close tab" }, h(Icon, { name: "close", size: 12 })));
      }),
      h("button", { type: "button", className: "ef-tab-add", "aria-label": "New tab", title: "New tab (Ctrl+T)" }, h(Icon, { name: "plus", size: 14 })));
  }

  function Toolbar(p) {
    var scope = p.scope || "folder";
    return h("div", { className: "ef-toolbar", role: "toolbar", "aria-label": "Navigation" },
      h(IconButton, { icon: "arrow-left", label: "Back (Alt+Left)" }),
      h(IconButton, { icon: "arrow-right", label: "Forward (Alt+Right)", disabled: true }),
      h(IconButton, { icon: "arrow-up", label: "Parent folder (Alt+Up)" }),
      h(IconButton, { icon: "refresh", label: "Reload (F5)" }),
      h("span", { className: "ef-toolbar-sep" }),
      h(PathBar, { segments: p.segments || [], animateLast: true }),
      h(SearchScope, { value: scope }),
      h(SearchField, { placeholder: scope === "everywhere" ? "Search everywhere" : "Search this folder", defaultValue: p.query }),
      h("span", { className: "ef-toolbar-sep" }),
      h(IconButton, { icon: "list", label: "List view (Ctrl+1)", pressed: !p.grid }),
      h(IconButton, { icon: "grid", label: "Grid view (Ctrl+2)", pressed: !!p.grid }),
      h(IconButton, { icon: "columns", label: p.dual ? "Single pane (F3)" : "Dual pane (F3)", pressed: !!p.dual }),
      h(IconButton, { icon: "sidebar", label: "Preview pane (Space)", pressed: !!p.preview }),
      h(IconButton, { icon: p.hidden ? "eye" : "eye-off", label: p.hidden ? "Hide hidden files (Ctrl+H)" : "Show hidden files (Ctrl+H)", pressed: !!p.hidden }),
      h(IconButton, { icon: "command", label: "Command palette (Ctrl+K)" }),
      h(IconButton, { icon: "settings", label: "Settings (Ctrl+,)", pressed: !!p.settings }));
  }

  /** Where the search box looks: this folder, or everything the index covers (Ctrl+E). */
  function SearchScope(p) {
    return h(SegmentedControl, { label: "Search in", value: p.value || "folder", onChange: p.onChange, options: [
      { value: "folder", text: "Folder", title: "Search this folder (Ctrl+E)" },
      { value: "everywhere", text: "Everywhere", title: "Search everywhere (Ctrl+E)" }] });
  }

  function Sidebar(p) {
    return h("nav", { className: "ef-sidebar", "aria-label": "Places" }, p.children);
  }
  function SidebarSection(p) {
    return h("section", null,
      h("div", { className: "ef-sec-head" },
        p.world ? h("span", { className: "ef-sec-mark", style: { background: "var(--world-" + p.world + ")" } }) : null,
        p.title, p.count !== undefined ? h("span", { className: "ef-sec-count" }, p.count) : null),
      h("div", { className: "ef-sec-list" }, p.children));
  }
  function SidebarItem(p) {
    return h("button", { type: "button", className: cx("ef-side-item", p.dropTarget && "ef-side-drop", p.indent && "ef-side-indent"), "aria-current": p.active ? "true" : undefined },
      h(Icon, { name: p.icon || "folder", size: 16 }),
      h("span", { className: "ef-side-name" }, p.label),
      p.trail ? h("span", { className: "ef-side-trail" }, p.trail) : null);
  }

  function UsageBar(p) {
    var pct = Math.max(0, Math.min(100, p.value || 0));
    var color = p.color || (p.world ? "var(--world-" + p.world + ")" : "var(--accent)");
    if (pct >= 97) color = "var(--danger)"; else if (pct >= 90) color = "var(--warning)";
    return h("div", { className: cx("ef-usage", p.large && "ef-usage-lg"), role: "meter", "aria-valuenow": pct, "aria-valuemin": 0, "aria-valuemax": 100, "aria-label": p.label || "Used space" },
      h("i", { style: { width: pct + "%", background: color } }));
  }

  var STATE = {
    mounted: ["success", "Mounted"], readonly: ["warning", "Read-only"], dirty: ["warning", "Needs check"],
    locked: ["danger", "Locked"], unmounted: [null, "Not mounted"], mounting: ["accent", "Mounting…"],
    cloud: ["info", "Cloud-only"], ads: ["info", "+ADS"], hidden: [null, "Hidden"], system: [null, "System"]
  };
  function StatePill(p) {
    var s = STATE[p.state] || [p.tone, p.children];
    return h("span", { className: cx("ef-pill", s[0] && "ef-pill-" + s[0]) }, p.children || s[1]);
  }

  function DriveItem(p) {
    var off = p.state === "unmounted" || p.state === "locked";
    return h("button", { type: "button", className: cx("ef-drive", off && "ef-drive-off"), "aria-current": p.active ? "true" : undefined },
      h(Icon, { name: p.icon || "drive", size: 16 }),
      h("span", { className: "ef-drive-name" }, p.name),
      p.state === "mounting" ? h("span", { className: "ef-spinner", role: "status", "aria-label": "Mounting" }) :
        (p.state === "locked" ? h(Icon, { name: "lock", size: 14, className: "ef-drive-lock" }) : h("span")),
      !off && p.used !== undefined ? h(UsageBar, { value: p.used, world: p.world }) : null,
      h("span", { className: "ef-drive-meta" },
        h("span", { className: "ef-drive-meta-l" }, p.state && p.state !== "mounted" && p.state !== "mounting" ? h(StatePill, { state: p.state }) : null, p.meta),
        p.free ? h("span", null, p.free) : null));
  }

  // ------------------------------------------------------------ files
  function splitExt(name, kind) {
    if (kind === "folder") return [name, ""];
    var i = name.lastIndexOf(".");
    return i > 0 ? [name.slice(0, i), name.slice(i)] : [name, ""];
  }

  function FileRow(p) {
    var f = p.file;
    var parts = splitExt(f.name, f.kind);
    var name = p.renaming
      ? h("input", {
          className: "ef-rename", defaultValue: f.name, autoFocus: true, "aria-label": "New name",
          onFocus: function (e) { e.target.setSelectionRange(0, parts[0].length); }
        })
      : h("span", { className: "ef-file-name" }, parts[0], parts[1] ? h("span", { className: "ef-ext" }, parts[1]) : null);
    return h("div", {
      className: "ef-file", role: "row", "aria-selected": String(!!f.selected),
      "data-cursor": f.cursor ? "true" : undefined, "data-cut": f.cut ? "true" : undefined,
      "data-hidden": f.hidden ? "true" : undefined, "data-drop": f.drop ? "true" : undefined,
      "data-removing": f.removing ? "true" : undefined,
      onClick: p.onClick
    },
      h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 18, badge: f.badge }),
      name,
      h("span", { className: "ef-file-meta ef-file-num", role: "cell" }, f.size || (f.kind === "folder" ? (f.items !== undefined ? f.items + " items" : "—") : "")),
      h("span", { className: "ef-file-meta", role: "cell" }, f.type || ""),
      h("span", { className: "ef-file-meta", role: "cell" }, f.modified || ""));
  }

  function ColumnHeader(p) {
    var cols = [["name", "Name"], ["size", "Size"], ["type", "Kind"], ["modified", "Modified"]];
    return h("div", { className: "ef-list-head", role: "row" },
      h("span"),
      cols.map(function (c) {
        var on = p.sort === c[0];
        return h("button", { key: c[0], type: "button", role: "columnheader", className: cx("ef-colh", c[0] === "size" && "ef-colh-num"), "aria-sort": on ? (p.desc ? "descending" : "ascending") : undefined },
          c[1], on ? h(Icon, { name: p.desc ? "arrow-down" : "arrow-up", size: 12, strokeWidth: 2.5 }) : null);
      }));
  }

  function Skeleton(p) {
    var n = p.rows || 6, rows = [];
    for (var i = 0; i < n; i++) {
      rows.push(h("div", { className: "ef-skel", key: i, "aria-hidden": "true" },
        h("i"), h("i", { style: { width: (40 + ((i * 37) % 45)) + "%" } }), h("i", { style: { width: "70%", justifySelf: "end" } }), h("i", { style: { width: "60%" } }), h("i", { style: { width: "80%" } })));
    }
    return h("div", { role: "status", "aria-label": "Loading folder" }, rows);
  }

  function FileList(p) {
    var st = useState(p.cursor !== undefined ? p.cursor : -1);
    var files = p.files || [];
    return h("div", { className: cx("ef-list", p.density === "compact" && "ef-dense", p.density === "comfortable" && "ef-comfy"), role: "grid", "aria-label": p.label || "Files", "aria-multiselectable": "true", tabIndex: 0 },
      h(ColumnHeader, { sort: p.sort || "name", desc: p.desc }),
      p.loading ? h(Skeleton, { rows: p.loading }) : files.map(function (f, i) {
        return h(FileRow, { key: f.name, file: Object.assign({}, f, st[0] === i ? { cursor: true } : null), renaming: p.renaming === i, onClick: function () { st[1](i); } });
      }));
  }

  function FileTile(p) {
    var f = p.file;
    return h("div", { className: "ef-tile", role: "gridcell", "aria-selected": String(!!f.selected), "data-cursor": f.cursor ? "true" : undefined, tabIndex: -1 },
      h("div", { className: "ef-tile-art" },
        f.thumb ? h("img", { className: "ef-thumb", src: f.thumb, alt: "" }) : h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 48, badge: f.badge })),
      h("span", { className: "ef-tile-name", title: f.name }, f.name),
      f.sub ? h("span", { className: "ef-tile-sub" }, f.sub) : null);
  }
  function FileGrid(p) {
    return h("div", { className: "ef-grid", role: "grid", "aria-label": p.label || "Files" },
      (p.files || []).map(function (f) { return h(FileTile, { key: f.name, file: f }); }));
  }

  function StatusBar(p) {
    return h("footer", { className: "ef-status", role: "status" },
      h("span", null, h("strong", null, p.count), " items"),
      p.selected ? h("span", { className: "ef-status-sep" }) : null,
      p.selected ? h("span", null, h("strong", null, p.selected), " selected", p.selectedSize ? " · " + p.selectedSize : "") : null,
      h("span", { className: "ef-status-grow" }),
      p.task ? h("span", { style: { display: "inline-flex", alignItems: "center", gap: 6 } }, h("span", { className: "ef-spinner", style: { width: 10, height: 10, borderWidth: 1.5 } }), p.task) : null,
      p.task ? h("span", { className: "ef-status-sep" }) : null,
      p.volume ? h("span", null, p.volume) : null,
      p.state ? h(StatePill, { state: p.state }) : null,
      p.free ? h("span", null, p.free) : null);
  }

  function EmptyState(p) {
    return h("div", { className: "ef-empty" },
      h(FileIcon, { name: p.icon || "folder-open", size: 64 }),
      h("h3", null, p.title || "This folder is empty"),
      p.body ? h("p", null, p.body) : null,
      p.actions ? h("div", { className: "ef-empty-actions" }, p.actions) : null);
  }

  function DriveCard(p) {
    return h("button", { type: "button", className: "ef-dcard" },
      h(FileIcon, { name: p.icon || "drive", size: 48 }),
      h("span", { className: "ef-dcard-name" }, p.name, p.state && p.state !== "mounted" ? h(StatePill, { state: p.state }) : null),
      h("span", { className: "ef-dcard-sub" }, p.fs),
      p.used !== undefined ? h(UsageBar, { value: p.used, world: p.world, large: true }) : null,
      h("span", { className: "ef-dcard-free" }, h("span", null, p.free), h("span", null, p.total)));
  }

  // ------------------------------------------------------------ feedback
  function Spinner(p) { return h("span", { className: cx("ef-spinner", p.large && "ef-spinner-lg"), role: "status", "aria-label": p.label || "Loading" }); }

  function Banner(p) {
    var icon = p.icon || (p.tone === "warning" ? "alert" : p.tone === "danger" ? "error" : "info");
    return h("div", { className: cx("ef-banner", p.tone && "ef-banner-" + p.tone), role: p.tone === "danger" ? "alert" : "status" },
      h(Icon, { name: icon, size: 16 }),
      h("div", { className: "ef-banner-text" }, p.children),
      p.actions ? h("div", { className: "ef-banner-actions" }, p.actions) : null,
      p.onClose !== false ? h(IconButton, { icon: "close", label: "Dismiss" }) : null);
  }

  function Toast(p) {
    return h("div", { className: cx("ef-toast", p.tone && "ef-toast-" + p.tone), role: "status" },
      h(Icon, { name: p.icon || (p.tone === "success" ? "check" : p.tone === "danger" ? "error" : "info"), size: 16 }),
      h("span", { className: "ef-toast-title" }, p.title),
      h(IconButton, { icon: "close", label: "Dismiss" }),
      p.children ? h("div", { className: "ef-toast-body" }, p.children) : null,
      p.actions ? h("div", { className: "ef-toast-actions" }, p.actions) : null);
  }

  function TransferToast(p) {
    var st = useState(p.value || 0);
    var v = st[0];
    var done = v >= 100;
    useEffect(function () {
      if (!p.animate || done) return;
      var t = setInterval(function () { st[1](function (x) { return Math.min(100, x + 1.5); }); }, 90);
      return function () { clearInterval(t); };
    }, [done]);
    return h("div", { className: cx("ef-toast", done && "ef-toast-success"), role: "status", "aria-live": "polite" },
      h(Icon, { name: done ? "check" : (p.icon || "copy"), size: 16 }),
      h("span", { className: "ef-toast-title" }, done ? (p.doneTitle || "Copied") : p.title),
      done ? h(IconButton, { icon: "close", label: "Dismiss" }) : h("span", { style: { display: "flex", gap: 2 } }, h(IconButton, { icon: "pause", label: "Pause" }), h(IconButton, { icon: "close", label: "Cancel (Esc)" })),
      h("div", { className: "ef-toast-body" }, p.detail),
      h("div", { className: cx("ef-progress", done && "ef-progress-done", p.indeterminate && "ef-progress-indet"), role: "progressbar", "aria-valuenow": Math.round(v), "aria-valuemin": 0, "aria-valuemax": 100 },
        h("i", { style: { width: v + "%" } })),
      h("div", { className: "ef-toast-meta" },
        h("span", null, done ? (p.doneMeta || "Flushed to disk") : p.meta),
        h("span", null, done ? "" : p.eta)));
  }

  function Dialog(p) {
    return h("div", { className: "ef-scrim", style: p.inline === false ? null : { minHeight: p.height || 300 } },
      h("div", { className: cx("ef-dialog", p.tone === "danger" && "ef-dialog-danger"), role: "dialog", "aria-modal": "true", "aria-labelledby": "dlg-title" },
        h("div", { className: "ef-dialog-head" },
          p.icon ? h(FileIcon, { name: p.icon, size: 32 }) : null,
          h("div", { style: { flex: 1, minWidth: 0 } }, h("h2", { className: "ef-dialog-title", id: "dlg-title" }, p.title))),
        h("div", { className: "ef-dialog-body" }, p.children),
        h("div", { className: "ef-dialog-foot" }, p.footer)));
  }

  function ConflictDialog(p) {
    return h(Dialog, {
      title: p.title || "“Report-Q3.xlsx” already exists in AVS (D:)", icon: "file-sheet", height: 420,
      footer: [
        h("span", { key: "all", className: "ef-grow" }, h(Checkbox, { id: "apply-all" }, "Do this for all " + (p.count || 12) + " conflicts")),
        h(Button, { key: "cancel", variant: "ghost", kbd: ["Esc"] }, "Cancel"),
        h(Button, { key: "skip" }, "Skip"),
        h(Button, { key: "rep" }, p.folders ? "Merge" : "Replace"),
        h(Button, { key: "both", variant: "primary", kbd: ["Enter"] }, "Keep both")]
    },
      h("div", { className: "ef-conflict" },
        h("div", { className: "ef-conflict-card" }, h(FileIcon, { name: "file-sheet", size: 32 }), h("b", null, "Copying"), h("span", { className: "ef-newer" }, "Modified today 18:22 · newer"), h("span", null, "84 KB · ~/Documents")),
        h("div", { className: "ef-conflict-card" }, h(FileIcon, { name: "file-sheet", size: 32 }), h("b", null, "Existing"), h("span", null, "Modified 2 Sep 09:10"), h("span", null, "81 KB · D:\\Work"))),
      h("p", null, "Keep both saves the copy as “Report-Q3 (2).xlsx”."));
  }

  function Tooltip(p) {
    return h("span", { className: "ef-tooltip", role: "tooltip" }, p.children, p.kbd ? h(Kbd, { keys: p.kbd }) : null);
  }

  function ContextMenu(p) {
    return h("div", { className: "ef-menu", role: "menu", "aria-label": p.label || "Actions" },
      (p.items || []).map(function (it, i) {
        if (it === "-") return h("div", { key: i, className: "ef-menu-sep", role: "separator" });
        if (it.heading) return h("div", { key: i, className: "ef-menu-head" }, it.heading);
        return h("button", { key: i, type: "button", role: "menuitem", className: cx("ef-mi", it.danger && "ef-mi-danger"), "data-active": it.active ? "true" : undefined, disabled: it.disabled },
          h(Icon, { name: it.icon || "chevron-right", size: 16 }),
          h("span", { className: "ef-mi-label" }, it.label),
          it.hint ? h("span", { className: "ef-mi-hint" }, it.hint) : null,
          it.kbd ? h(Kbd, { keys: it.kbd }) : null,
          it.submenu ? h(Icon, { name: "chevron-right", size: 12 }) : null);
      }));
  }

  function highlight(label, q) {
    if (!q) return label;
    var out = [], li = 0, qi = 0, lower = label.toLowerCase(), ql = q.toLowerCase();
    for (var i = 0; i < label.length; i++) {
      if (qi < ql.length && lower[i] === ql[qi]) { out.push(h("mark", { key: i }, label[i])); qi++; }
      else out.push(label[i]);
    }
    return out;
  }

  function CommandPalette(p) {
    var st = useState(p.query || "");
    var q = st[0];
    var groups = p.groups || [];
    return h("div", { className: "ef-palette", role: "dialog", "aria-label": "Command palette" },
      h("div", { className: "ef-palette-input" },
        h("span", { className: "ef-palette-prompt" }, ">"),
        h("input", { id: p.id || "palette-q", value: q, onChange: function (e) { st[1](e.target.value); }, placeholder: "Type a command, folder or drive…", "aria-label": "Command" }),
        h(Kbd, { keys: ["Esc"] })),
      h("div", { className: "ef-palette-list", role: "listbox" },
        groups.map(function (g, gi) {
          return h(React.Fragment, { key: gi },
            h("div", { className: "ef-menu-head" }, g.heading),
            g.items.map(function (it, i) {
              return h("button", { key: i, type: "button", role: "option", className: "ef-mi", "data-active": gi === 0 && i === 0 ? "true" : undefined, "aria-selected": gi === 0 && i === 0 ? "true" : undefined },
                h(Icon, { name: it.icon, size: 16 }),
                h("span", { className: "ef-mi-label" }, highlight(it.label, q)),
                it.hint ? h("span", { className: "ef-mi-hint" }, it.hint) : null,
                it.kbd ? h(Kbd, { keys: it.kbd }) : null);
            }));
        })),
      h("div", { className: "ef-palette-foot" },
        h("span", null, h(Kbd, { keys: ["↑", "↓"] }), "move"), h("span", null, h(Kbd, { keys: ["Enter"] }), "run"),
        h("span", null, h(Kbd, { keys: ["Tab"] }), "complete path"), h("span", null, h(Kbd, { keys: ["/"] }), "search files")));
  }

  // ------------------------------------------------------------ panels
  function PreviewPane(p) {
    var f = p.file || {};
    return h("aside", { className: "ef-preview", "aria-label": "Preview" },
      h("div", { className: "ef-preview-art" }, f.thumb ? h("img", { src: f.thumb, alt: "" }) : h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 96 })),
      h("div", { className: "ef-preview-body" },
        h("h2", { className: "ef-preview-title" }, f.name),
        h("dl", { className: "ef-props" }, (p.props || []).map(function (r, i) {
          return h(React.Fragment, { key: i }, h("dt", null, r[0]), h("dd", { title: r[1] }, r[1]));
        })),
        p.attrs ? h("div", null, h("div", { className: "ef-sec-head", style: { padding: 0, marginBottom: 6 } }, "Windows attributes"),
          h("div", { className: "ef-attrs" }, p.attrs.map(function (a) { return h(StatePill, { key: a, tone: null }, a); }))) : null,
        h("div", { className: "ef-preview-actions" },
          h(Button, { size: "sm", icon: "external" }, "Open"),
          h(Button, { size: "sm", icon: "copy" }, p.windowsPath ? "Copy Windows path" : "Copy path"))));
  }

  // ------------------------------------------------------------ pages
  var DEMO_FILES = [
    { name: "Projects", kind: "folder", items: 14, type: "Folder", modified: "Today 11:02" },
    { name: "Screenshots", kind: "folder", icon: "folder-image", items: 382, type: "Folder", modified: "Today 09:47" },
    { name: "Shortcut to Games", kind: "folder", badge: "link", type: "Junction", modified: "3 Aug" },
    { name: "Report-Q3.xlsx", size: "84 KB", type: "Spreadsheet", modified: "Today 18:22", selected: true },
    { name: "IMG_20260914_182233.jpg", size: "4.2 MB", type: "JPEG image", modified: "14 Sep 18:22", selected: true },
    { name: "Taxes-2025.pdf", size: "1.1 MB", type: "PDF", modified: "2 Sep", badge: "cloud" },
    { name: "main.rs", size: "12 KB", type: "Rust source", modified: "Yesterday" },
    { name: "Backup.7z", size: "2.4 GB", type: "7-Zip archive", modified: "21 Aug", badge: "ads" },
    { name: "desktop.ini", size: "282 B", type: "Settings", modified: "12 Jan", hidden: true },
    { name: "Old Launcher.lnk", size: "1 KB", type: "Shortcut", modified: "4 Mar", badge: "broken" }
  ];

  function DemoSidebar(p) {
    return h(Sidebar, null,
      h(SidebarSection, { title: "Linux", world: "linux" },
        h(SidebarItem, { icon: "home", label: "Home" }),
        h(SidebarItem, { icon: "file", label: "Documents" }),
        h(SidebarItem, { icon: "download", label: "Downloads", trail: "3 new" }),
        h(SidebarItem, { icon: "image", label: "Pictures" }),
        h(SidebarItem, { icon: "trash", label: "Trash" })),
      h(SidebarSection, { title: "Pinned" },
        h(SidebarItem, { icon: "pin", label: "Work" }),
        h(SidebarItem, { icon: "pin", label: "EchoFiles_Linux" })),
      h(SidebarSection, { title: "Windows", world: "windows", count: 3 },
        h(DriveItem, { name: "Windows (C:)", state: "readonly", used: 76, world: "windows", meta: "NTFS", free: "88 GB free", active: p.active === "c" }),
        h(SidebarItem, { icon: "folder", label: "Documents", indent: true }),
        h(SidebarItem, { icon: "folder", label: "Downloads", indent: true }),
        h(DriveItem, { name: "AVS (D:)", state: "mounted", used: 58, world: "windows", meta: "NTFS", free: "66 GB free", active: p.active === "d" }),
        h(DriveItem, { name: "AVS (E:)", state: "unmounted", meta: "295 GB · click to mount" })),
      h(SidebarItem, { icon: "sliders", label: "All drives" }));
  }

  function AppWindow(p) {
    return h("div", { className: "ef ef-app", style: { height: p.height || 640 } },
      h(TabStrip, { tabs: [{ label: "Work", icon: "folder" }, { label: "Downloads", icon: "download" }, { label: "Pictures", icon: "image" }], active: 0 }),
      h(Toolbar, { segments: [{ label: "AVS (D:)", icon: "drive" }, { label: "Work" }, { label: "2026" }], preview: true }),
      h("div", { className: "ef-app-main" },
        h(DemoSidebar, { active: "d" }),
        h("div", { className: "ef-app-center" },
          p.banner !== false ? h(Banner, { tone: "info", icon: "cloud", actions: h(Button, { size: "sm" }, "Learn more") },
            h("strong", null, "2 files are OneDrive placeholders."), " They open once Windows has synced them.") : null,
          h("div", { className: "ef-app-scroll" }, h(FileList, { files: DEMO_FILES, cursor: 4 }))),
        h(PreviewPane, {
          file: { name: "IMG_20260914_182233.jpg" },
          props: [["Kind", "JPEG image"], ["Size", "4.2 MB"], ["Dimensions", "4032 × 3024"], ["Modified", "14 Sep 2026 18:22"], ["Where", "D:\\Work\\2026"]],
          attrs: ["Archive"], windowsPath: true
        })),
      h(StatusBar, { count: "1,284", selected: 2, selectedSize: "4.3 MB", volume: "AVS (D:) · ntfs3", free: "66 GB free", task: "Copying 42%" }),
      h("div", { className: "ef-app-float" },
        h(TransferToast, { title: "Copying 1,204 files to AVS (D:)", detail: "From ~/Pictures/2026 · IMG_4471.HEIC", value: 42, animate: p.animate, meta: "812 MB of 2.4 GB · 94 MB/s", eta: "17 s left" })));
  }

  function DualPane(p) {
    var left = DEMO_FILES.slice(3, 8).map(function (f, i) { return Object.assign({}, f, { selected: i < 2, cursor: i === 1 }); });
    var right = DEMO_FILES.slice(0, 3).concat([{ name: "Report-Q3.xlsx", size: "81 KB", type: "Spreadsheet", modified: "2 Sep" }, { name: "Invoices", kind: "folder", items: 57, drop: true }]);
    function pane(title, icon, files, active, world) {
      return h("div", { className: cx("ef-pane", active ? "ef-pane-active" : "ef-pane-inactive") },
        h("div", { className: "ef-pane-head" },
          h("span", { className: "ef-sec-mark", style: { background: "var(--world-" + world + ")" } }),
          h(Icon, { name: icon, size: 14 }), h("strong", null, title), h("span", { className: "ef-muted", style: { marginLeft: "auto" } }, files.length + " items")),
        h("div", { className: "ef-list" },
          h("div", { className: "ef-list-head" }, h("span"), h("button", { className: "ef-colh", "aria-sort": "ascending", type: "button" }, "Name", h(Icon, { name: "arrow-up", size: 12 })), h("button", { className: "ef-colh ef-colh-num", type: "button" }, "Size")),
          files.map(function (f) {
            var parts = splitExt(f.name, f.kind);
            return h("div", { key: f.name, className: "ef-file", role: "row", "aria-selected": String(!!f.selected), "data-cursor": f.cursor ? "true" : undefined, "data-drop": f.drop ? "true" : undefined },
              h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 18, badge: f.badge }),
              h("span", { className: "ef-file-name" }, parts[0], parts[1] ? h("span", { className: "ef-ext" }, parts[1]) : null),
              h("span", { className: "ef-file-meta ef-file-num" }, f.size || (f.items + " items")));
          })));
    }
    return h("div", { className: "ef ef-app", style: { height: p.height || 420 } },
      h(Toolbar, { segments: [{ label: "Home", icon: "home" }, { label: "Documents" }], view: "columns" }),
      h("div", { className: "ef-dual" },
        pane("~/Documents", "home", left, true, "linux"),
        pane("AVS (D:) \\ Work", "drive", right, false, "windows")),
      h(StatusBar, { count: "5", selected: 2, selectedSize: "4.3 MB", volume: "F5 copy → AVS (D:)  ·  F6 move" }));
  }

  function MotionCell(p) {
    var st = useState(0);
    return h("div", { className: "ef-motion-cell" },
      h("div", { className: "ef-motion-stage", key: st[0] }, p.stage),
      h("b", null, p.title), h("span", null, p.note),
      h("button", { type: "button", className: "ef-btn ef-btn-sm ef-btn-ghost ef-replay", onClick: function () { st[1](st[0] + 1); } }, h(Icon, { name: "refresh", size: 12 }), "Replay"));
  }

  function MotionSpec() {
    var ghost = h("div", { className: "ef-ghost" },
      h(FileIcon, { name: "file-image", size: 36, className: "", key: 1 }),
      h("span", { style: { position: "absolute", left: 5, top: 5 } }, h(FileIcon, { name: "file-sheet", size: 36 })),
      h("span", { style: { position: "absolute", left: 10, top: 10 } }, h(FileIcon, { name: "file-pdf", size: 36 })),
      h("span", { className: "ef-ghost-count" }, "12"));
    return h("div", { className: "ef ef-motion" },
      h(MotionCell, { title: "Hover · 90ms", note: "State layer fades in. Never scales, never lifts.", stage: h(Button, null, "Hover me") }),
      h(MotionCell, { title: "Selection · 0ms", note: "Cursor and selection are instant. Lists never animate on keyboard input.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "main.rs", size: "12 KB", selected: true, cursor: true } })) }),
      h(MotionCell, { title: "Breadcrumb · 140ms", note: "Navigation is instant; only the new segment slides in.", stage: h(PathBar, { segments: [{ label: "Home" }, { label: "Pictures" }], animateLast: true }) }),
      h(MotionCell, { title: "Drop target · spring", note: "Folder pops, fills accent-soft; spring-load bar opens it after 700ms.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "Invoices", kind: "folder", items: 57, drop: true } })) }),
      h(MotionCell, { title: "Drag ghost", note: "Up to three icons stacked like the logo's sheets, plus a count.", stage: ghost }),
      h(MotionCell, { title: "Toast · 220ms spring", note: "Rises 12px. Exits with a 150ms fade.", stage: h("div", { className: "ef-motion-mini" }, h(Toast, { title: "Moved to Trash", tone: "success", actions: h(Button, { size: "sm" }, "Undo") })) }),
      h(MotionCell, { title: "Menu · 140ms", note: "Drops 4px from the pointer side.", stage: h("div", { className: "ef-motion-mini ef-motion-mini-menu" }, h(ContextMenu, { items: [{ icon: "external", label: "Open", active: true }, { icon: "rename", label: "Rename" }] })) }),
      h(MotionCell, { title: "Usage bar · 400ms", note: "Grows once when a drive mounts; then static.", stage: h("div", { style: { width: "80%" } }, h(UsageBar, { value: 64, world: "windows", large: true })) }),
      h(MotionCell, { title: "Skeleton · after 150ms", note: "Only shown if a folder takes longer than 150ms. Shimmer loops 1.4s.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(Skeleton, { rows: 2 })) }),
      h(MotionCell, { title: "Delete · 140ms", note: "Row collapses with ease-exit; rows below slide up. Undo toast follows.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "old-notes.txt", size: "2 KB", removing: true } })) }),
      h(MotionCell, { title: "Thumbnail · 140ms", note: "Decoded thumbnails fade over the icon. No layout shift.", stage: h("div", { className: "ef-tile-art", style: { width: 56, height: 56 } }, h("div", { className: "ef-thumb", style: { width: 56, height: 42, background: "linear-gradient(135deg, var(--world-windows), var(--success))" } })) }),
      h(MotionCell, { title: "Progress", note: "Linear fill; turns success and draws a tick at 100%.", stage: h("div", { className: "ef-motion-mini" }, h(TransferToast, { title: "Copying 3 files", detail: "To AVS (D:)", value: 70, animate: true, meta: "", eta: "" })) }));
  }

  // ------------------------------------------------------------ settings
  // ------------------------------------------------------------ settings
  /** Uppercase label above a bordered box of rows separated by hairlines. */
  function SettingsGroup(p) {
    return h("section", { className: "ef-setg" },
      h("h3", { className: "ef-setg-label" }, p.title),
      h("div", { className: "ef-setbox" }, p.children));
  }

  /** One setting: what it does on the left, its control on the right. */
  function SettingRow(p) {
    return h("div", { className: cx("ef-setrow", p.disabled && "ef-setrow-off") },
      h("div", { className: "ef-setrow-text" },
        h("span", { className: "ef-setrow-label" }, p.label),
        p.description ? h("span", { className: "ef-setrow-desc" }, p.description) : null),
      h("div", { className: "ef-setrow-control" }, p.children));
  }

  /** A full-width row inside a group: notes, lists, code. */
  function SettingBlock(p) {
    return h("div", { className: cx("ef-setblock", p.muted && "ef-setrow-desc") }, p.children);
  }

  function PathListEditor(p) {
    var st = useState(p.paths || []);
    var input = useState(p.draft || "");
    var paths = st[0];
    return h(React.Fragment, null,
      paths.length === 0 ? h(SettingBlock, { muted: true }, p.empty || "Nothing here yet.") :
        paths.map(function (path, i) {
          return h("div", { key: path, className: "ef-plist-row" },
            h(Icon, { name: p.icon || "folder", size: 16 }),
            h("span", { className: "ef-plist-path", title: path }, path),
            p.note && p.note[i] ? h(StatePill, { tone: "warning" }, p.note[i]) : null,
            h(IconButton, { icon: "close", label: "Remove " + path, onClick: function () { st[1](paths.filter(function (_, k) { return k !== i; })); } }));
        }),
      h("div", { className: "ef-setblock" },
        h("div", { className: "ef-plist-add" },
          h("div", { className: cx("ef-field", p.error && "ef-field-invalid"), style: { flex: 1 } },
            h("input", { id: p.id, value: input[0], placeholder: p.placeholder || "Folder path, like ~/Projects", "aria-label": "Folder to add", "aria-invalid": p.error ? "true" : undefined, onChange: function (e) { input[1](e.target.value); } })),
          h(Button, { icon: "plus", onClick: function () { if (input[0].trim()) { st[1](paths.concat([input[0].trim()])); input[1](""); } } }, "Add"),
          p.current ? h(Button, { variant: "ghost", icon: "folder-plus", onClick: function () { st[1](paths.concat([p.current])); } }, "Add current folder") : null),
        p.error ? h("div", { className: "ef-field-msg", role: "alert" }, h(Icon, { name: "error", size: 14 }), p.error) : null));
  }

  function NameChips(p) {
    var st = useState(p.names || []);
    var input = useState("");
    var names = st[0];
    function add() { if (input[0].trim()) { st[1](names.concat([input[0].trim()])); input[1](""); } }
    return h(React.Fragment, null,
      h(SettingBlock, null, h("div", { className: "ef-chips" },
        names.map(function (n, i) {
          return h("span", { key: n, className: "ef-namechip" }, n,
            h("button", { type: "button", "aria-label": "Stop skipping " + n, onClick: function () { st[1](names.filter(function (_, k) { return k !== i; })); } }, h(Icon, { name: "close", size: 12 })));
        }))),
      h(SettingBlock, null, h("div", { className: "ef-plist-add" },
        h("div", { className: "ef-field", style: { flex: 1 } },
          h("input", { id: p.id, value: input[0], placeholder: "Folder name, like node_modules", "aria-label": "Folder name to skip", onChange: function (e) { input[1](e.target.value); }, onKeyDown: function (e) { if (e.key === "Enter") add(); } })),
        h(Button, { icon: "plus", onClick: add }, "Add"),
        h(Button, { variant: "ghost", icon: "undo" }, "Reset to recommended"))));
  }

  var INDEX = {
    off: [null, "Off", "Everywhere search walks your folders live instead — slower on big folders."],
    opening: ["info", "Opening", "Loading the saved index…"],
    building: ["info", "Indexing", "Building the index for the first time. Search works live meanwhile."],
    updating: ["info", "Updating", "179,581 items · 13 MB · updating now"],
    ready: ["success", "Ready", "179,581 items · 13 MB on disk · updated 21 s ago"],
    problem: ["warning", "Problem", "Couldn't index ~/Work: permission denied."]
  };
  /** Health of the search index, with the one action that helps. */
  function IndexStatus(p) {
    var s = INDEX[p.state || "ready"];
    var busy = p.state === "building" || p.state === "updating";
    return h("div", { className: "ef-idx" },
      h(StatePill, { tone: s[0] }, s[1]),
      h("span", { className: "ef-idx-detail" }, p.detail || s[2]),
      h(Button, { icon: "refresh", disabled: busy || p.state === "off" }, busy ? "Indexing…" : "Rebuild now"));
  }

  /** Commands or shortcuts with what they do, on the bg-deep well. */
  function CommandList(p) {
    return h("div", { className: "ef-cmds" }, (p.items || []).map(function (it, i) {
      return h("div", { key: i, className: "ef-cmd" }, h("code", null, it[0]), h("span", null, it[1]));
    }));
  }

  var SETTINGS_PAGES = [
    { id: "general", icon: "sliders", label: "General" },
    { id: "search", icon: "search", label: "Search & index" },
    { id: "agents", icon: "terminal", label: "AI agents" },
    { id: "appearance", icon: "image", label: "Appearance" },
    { id: "about", icon: "info", label: "About" }];

  function SettingsNav(p) {
    return h("nav", { className: "ef-sidebar ef-setnav", "aria-label": "Settings pages" },
      SETTINGS_PAGES.map(function (pg) {
        return h("button", { key: pg.id, type: "button", className: "ef-side-item ef-setnav-item", "aria-current": pg.id === p.page ? "true" : undefined, onClick: function () { p.onChange && p.onChange(pg.id); } },
          h(Icon, { name: pg.icon, size: 16 }), h("span", { className: "ef-side-name" }, pg.label));
      }));
  }

  /** Settings as its own screen: top bar back to Files, page nav, one page of groups. */
  function SettingsShell(p) {
    var saved = p.saved !== false;
    return h("div", { className: "ef ef-settings", style: { height: p.height } },
      h("div", { className: "ef-toolbar ef-settings-bar" },
        h(Button, { variant: "ghost", icon: "arrow-left", kbd: ["Esc"] }, "Files"),
        h("span", { className: "ef-toolbar-sep" }),
        h("strong", { className: "ef-settings-title" }, "Settings"),
        h("span", { className: "ef-status-grow" }),
        h("span", { className: saved ? "ef-saved" : "ef-muted" }, h(Icon, { name: saved ? "check" : "clock", size: 14 }), saved ? "Saved" : "Saving…")),
      h("div", { className: "ef-settings-body" },
        h(SettingsNav, { page: p.page, onChange: p.onPage }),
        h("div", { className: "ef-settings-scroll" }, h("div", { className: "ef-settings-inner" }, p.children))));
  }

  function PageHead(p) {
    return h("header", { className: "ef-settings-head" }, h("h2", null, p.title), h("p", null, p.children));
  }

  function settingsPage(page) {
    if (page === "general") return [
      h(PageHead, { key: "h", title: "General" }, "How EchoFiles starts, runs and opens."),
      h(SettingsGroup, { key: "a", title: "Running" },
        h(SettingRow, { label: "Keep running in the background", description: "Closing the window hides EchoFiles instead of quitting. The next window opens instantly and search stays up to date." }, h(Switch, { defaultChecked: true, label: "Keep running in the background" })),
        h(SettingRow, { label: "Start at login", description: "Starts hidden when you log in, so even the first window is instant." }, h(Switch, { label: "Start at login" }))),
      h(SettingsGroup, { key: "b", title: "Windows" },
        h(SettingRow, { label: "New windows open at", description: "Where EchoFiles starts when you open it without a folder." }, h(SegmentedControl, { label: "Open at", value: "home", options: [{ value: "home", text: "Home" }, { value: "last", text: "Last folder" }] })),
        h(SettingRow, { label: "Show hidden files", description: "Dotfiles and dot-folders. Ctrl+H switches it for the open window." }, h(Switch, { label: "Show hidden files" })))];
    if (page === "agents") return [
      h(PageHead, { key: "h", title: "AI agents" }, "Let AI agents on this computer use EchoFiles' index. Nothing leaves your computer."),
      h(SettingsGroup, { key: "a", title: "Command line" },
        h(SettingRow, { label: "Allow EchoFiles commands", description: "Lets AI agents and scripts search your files with ef find — milliseconds instead of walking the disk. When off, ef refuses and says it's turned off." }, h(Switch, { defaultChecked: true, label: "Allow EchoFiles commands" })),
        h(SettingRow, { label: "ef command", description: "Installed on your PATH." }, h("span", { className: "ef-muted" }, "~/.local/bin/ef"))),
      h(SettingsGroup, { key: "b", title: "Skill" },
        h(SettingRow, { label: "Teach AI agents about ef", description: "Links the EchoFiles skill into ~/.claude/skills so agents like Claude Code reach for ef before find. Turning it off removes only that link." }, h(Switch, { label: "Teach AI agents about ef" }))),
      h(SettingsGroup, { key: "c", title: "Commands" }, h(SettingBlock, null, h(CommandList, { items: [
        ["ef find report", "names containing “report”"], ["ef find '*' --ext pdf --limit 50", "every PDF, first 50"],
        ["ef find invoice --in ~/Documents", "only inside a folder"], ["ef status", "what's indexed, how fresh"], ["ef index", "update the index now"]] })))];
    if (page === "appearance") return [
      h(PageHead, { key: "h", title: "Appearance" }, "EchoFiles looks like the rest of your desktop."),
      h(SettingsGroup, { key: "a", title: "Theme" },
        h(SettingRow, { label: "Omarchy theme · tokyo-night", description: "EchoFiles follows your Omarchy theme and font and switches with them. Change the theme from the Omarchy menu." },
          h("span", { className: "ef-swatches" }, ["--bg", "--bg-raised", "--ink", "--accent", "--world-linux", "--world-windows", "--success", "--warning", "--danger"].map(function (v) { return h("i", { key: v, style: { background: "var(" + v + ")" } }); })))),
      h(SettingsGroup, { key: "b", title: "Layout" },
        h(SettingRow, { label: "Row height", description: "How tightly the file list is packed." }, h(SegmentedControl, { label: "Row height", value: "default", options: [{ value: "compact", text: "Compact" }, { value: "default", text: "Default" }, { value: "comfortable", text: "Comfortable" }] })))];
    if (page === "about") return [
      h(PageHead, { key: "h", title: "About" }, "Where EchoFiles keeps things, and how to drive it from the keyboard."),
      h(SettingsGroup, { key: "a", title: "EchoFiles" },
        h(SettingRow, { label: "Version", description: "EchoFiles 0.1.0 · Linux" }),
        h(SettingRow, { label: "Settings file", description: "~/.config/echofiles/settings.toml" }, h(Button, { variant: "ghost", icon: "folder" }, "Show")),
        h(SettingRow, { label: "Index files", description: "~/.cache/echofiles/index" }, h(Button, { variant: "ghost", icon: "folder" }, "Show"))),
      h(SettingsGroup, { key: "b", title: "Keyboard" }, h(SettingBlock, null, h(CommandList, { items: [
        ["Ctrl+F  /", "search"], ["Ctrl+E", "search this folder ↔ everywhere"], ["Esc", "clear search · leave Settings"],
        ["Alt+← Alt+→ Alt+↑", "back · forward · parent"], ["Ctrl+H", "show hidden files"], ["F5", "reload"], ["Ctrl+,", "settings"]] })))];
    return [
      h(PageHead, { key: "h", title: "Search & index" }, "What search covers, and the index that makes it instant."),
      h(SettingsGroup, { key: "a", title: "Index" },
        h(SettingRow, { label: "Search index", description: "A list of file names kept on disk so Everywhere search answers instantly and AI agents can use ef. Updates within seconds in Downloads, Desktop and Documents and every minute elsewhere. Turning it off deletes the index." }, h(Switch, { defaultChecked: true, label: "Search index" })),
        h(IndexStatus, { state: "ready" })),
      h(SettingsGroup, { key: "b", title: "Search box" },
        h(SettingRow, { label: "Search looks in", description: "What the search box searches when a window opens. Ctrl+E switches it any time." }, h(SegmentedControl, { label: "Default scope", value: "folder", options: [{ value: "folder", text: "This folder" }, { value: "everywhere", text: "Everywhere" }] }))),
      h(SettingsGroup, { key: "c", title: "Indexed folders" },
        h(SettingBlock, { muted: true }, "Everywhere search and ef cover these folders and everything inside them."),
        h(PathListEditor, { id: "roots", paths: ["~"], current: "~/Projects/EchoFiles_Linux" })),
      h(SettingsGroup, { key: "d", title: "Never show in search" },
        h(SettingBlock, { muted: true }, "Folders that never appear in search results — private or noisy places. Their contents aren't indexed at all."),
        h(PathListEditor, { id: "excl", paths: [], empty: "Nothing excluded.", placeholder: "Folder path, like ~/Private", current: "~/Projects/EchoFiles_Linux" })),
      h(SettingsGroup, { key: "e", title: "Skip contents of" },
        h(SettingRow, { label: "Skip cache folders", description: "Folders marked with CACHEDIR.TAG (browser, build and package caches). They're still listed by name." }, h(Switch, { defaultChecked: true, label: "Skip cache folders" })),
        h(SettingBlock, { muted: true }, "Folders with these names are listed but their contents aren't indexed — package stores, build output, version control internals."),
        h(NameChips, { id: "names", names: ["node_modules", ".git", ".hg", ".svn", "__pycache__", ".cache", ".npm", ".pnpm-store", ".yarn", ".gradle", ".m2", ".cargo", ".rustup", ".venv", ".tox", ".mypy_cache", ".pytest_cache", ".next", ".nuxt"] }))];
  }

  function SettingsPage(p) {
    var st = useState(p.page || "search");
    return h(SettingsShell, { height: p.height, saved: p.saved, page: st[0], onPage: st[1] }, settingsPage(st[0]));
  }

  // ------------------------------------------------------------ search results
  var DEMO_HITS = [
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux", size: "1.3 KB", date: "Today 00:02" },
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux/crates/core", size: "352 B", date: "Yesterday" },
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux/crates/index", size: "460 B", date: "Yesterday" },
    { name: "Cargo.toml", kind: "file", location: "~/Downloads/t3code-0.0.39-nightly.20260902.1260/native/resource-monitor", size: "298 B", date: "2 Sep" },
    { name: "cargo", kind: "folder", location: "~/.local/share", size: "—", date: "9 Sep" }];

  /** "Everywhere" results: name, where it lives, size, date — answered from the index. */
  function SearchResults(p) {
    var hits = p.hits || DEMO_HITS;
    var st = useState(0);
    return h("div", { className: "ef-results", role: "grid", "aria-label": "Search results" },
      h("div", { className: "ef-results-row ef-results-head", role: "row" }, h("span"), h("span", null, "Name"), h("span", null, "Location"), h("span", { className: "ef-num" }, "Size"), h("span", null, "Modified")),
      h("div", { className: "ef-results-body" }, hits.map(function (f, i) {
        return h("div", { key: i, role: "row", className: "ef-results-row", "aria-selected": String(i === st[0]), onClick: function () { st[1](i); } },
          h(FileIcon, { name: iconFor(f.name, f.kind), size: 18 }),
          h("span", { className: "ef-results-name" }, f.name),
          h("span", { className: "ef-results-loc", title: f.location }, f.location),
          h("span", { className: "ef-num" }, f.size), h("span", null, f.date));
      })),
      h("div", { className: "ef-results-foot" },
        h(Kbd, { keys: ["Enter"] }), h("span", null, "open"), h(Kbd, { keys: ["Alt", "Enter"] }), h("span", null, "show in folder"),
        h("span", { className: "ef-status-grow" }),
        h(Button, { icon: "folder" }, "Show in folder")));
  }

  window.Echo = {
    Icon: Icon, FileIcon: FileIcon, Button: Button, IconButton: IconButton, SegmentedControl: SegmentedControl,
    Switch: Switch, Checkbox: Checkbox, Kbd: Kbd, TextField: TextField, SearchField: SearchField, PathBar: PathBar,
    TabStrip: TabStrip, Toolbar: Toolbar, Sidebar: Sidebar, SidebarSection: SidebarSection, SidebarItem: SidebarItem,
    DriveItem: DriveItem, UsageBar: UsageBar, StatePill: StatePill, FileRow: FileRow, ColumnHeader: ColumnHeader,
    FileList: FileList, Skeleton: Skeleton, FileTile: FileTile, FileGrid: FileGrid, StatusBar: StatusBar,
    EmptyState: EmptyState, DriveCard: DriveCard, Spinner: Spinner, Banner: Banner, Toast: Toast,
    TransferToast: TransferToast, Dialog: Dialog, ConflictDialog: ConflictDialog, Tooltip: Tooltip,
    ContextMenu: ContextMenu, CommandPalette: CommandPalette, PreviewPane: PreviewPane, AppWindow: AppWindow,
    DualPane: DualPane, MotionSpec: MotionSpec, SettingsGroup: SettingsGroup, PathListEditor: PathListEditor, NameChips: NameChips, SettingRow: SettingRow, SettingBlock: SettingBlock, SettingsPage: SettingsPage,
    SettingsShell: SettingsShell, SettingsNav: SettingsNav, IndexStatus: IndexStatus, CommandList: CommandList, SearchResults: SearchResults, SearchScope: SearchScope, iconFor: iconFor, DEMO_FILES: DEMO_FILES
  };
})();

})();
