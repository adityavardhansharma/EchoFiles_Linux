/* @ds-bundle: {"format":4,"namespace":"EchoConnect","components":[{"name":"Icon"},{"name":"FileIcon"},{"name":"Button"},{"name":"IconButton"},{"name":"Switch"},{"name":"Checkbox"},{"name":"SegmentedControl"},{"name":"TextField"},{"name":"StatePill"},{"name":"ProgressBar"},{"name":"Spinner"},{"name":"Snackbar"},{"name":"Banner"},{"name":"AppBar"},{"name":"BottomNav"},{"name":"ListGroup"},{"name":"ListRow"},{"name":"BottomSheet"},{"name":"Dialog"},{"name":"PhoneFrame"},{"name":"LaptopDevice"},{"name":"BatteryMeter"},{"name":"LinkPills"},{"name":"LaptopCard"},{"name":"ActionGrid"},{"name":"ClipItem"},{"name":"ClipModeCard"},{"name":"TransferRow"},{"name":"PermissionRow"},{"name":"StepList"},{"name":"Stepper"},{"name":"QrViewfinder"},{"name":"PairCode"},{"name":"CallBar"},{"name":"SystemNotification"},{"name":"QuickTile"},{"name":"SelectionMenu"},{"name":"SystemDialog"},{"name":"HomeScreen"},{"name":"ClipboardScreen"},{"name":"TransfersScreen"},{"name":"SettingsScreen"},{"name":"PermissionsScreen"},{"name":"ClipboardSetupScreen"},{"name":"PairingScreen"},{"name":"RingScreen"},{"name":"ShareSheetScreen"},{"name":"SystemSurfaces"},{"name":"AppMap"}]} */
(function(){
var ICONS = {"glyphs":{"arrow-left":"<path d=\"M19 12H5M11 6l-6 6 6 6\"/>","arrow-right":"<path d=\"M5 12h14M13 6l6 6-6 6\"/>","arrow-up":"<path d=\"M12 19V5M6 11l6-6 6 6\"/>","arrow-down":"<path d=\"M12 5v14M6 13l6 6 6-6\"/>","chevron-left":"<path d=\"M15 6l-6 6 6 6\"/>","chevron-right":"<path d=\"M9 6l6 6-6 6\"/>","chevron-up":"<path d=\"M6 15l6-6 6 6\"/>","chevron-down":"<path d=\"M6 9l6 6 6-6\"/>","home":"<path d=\"M4 10.5L12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z\"/>","history":"<path d=\"M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5M3.5 4v4.5H8M12 8v4.5l3 2\"/>","grid":"<rect x=\"4\" y=\"4\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"13.5\" y=\"4\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"4\" y=\"13.5\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/><rect x=\"13.5\" y=\"13.5\" width=\"6.5\" height=\"6.5\" rx=\"1.5\"/>","list":"<path d=\"M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01\"/>","columns":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M12 4v16\"/>","sidebar":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M9 4v16\"/>","sort":"<path d=\"M7 4v16M3.5 7.5L7 4l3.5 3.5M17 20V4M13.5 16.5L17 20l3.5-3.5\"/>","filter":"<path d=\"M4 5h16l-6 7.5V19l-4-2v-4.5z\"/>","sliders":"<path d=\"M4 7h9M17 7h3M4 17h3M11 17h9\"/><circle cx=\"15\" cy=\"7\" r=\"2\"/><circle cx=\"9\" cy=\"17\" r=\"2\"/>","more-vertical":"<path d=\"M12 5h.01M12 12h.01M12 19h.01\" stroke-width=\"3\"/>","more-horizontal":"<path d=\"M5 12h.01M12 12h.01M19 12h.01\" stroke-width=\"3\"/>","search":"<circle cx=\"11\" cy=\"11\" r=\"6.5\"/><path d=\"M16 16l4.5 4.5\"/>","eye":"<path d=\"M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/>","eye-off":"<path d=\"M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/><path d=\"M4 4l16 16\"/>","copy":"<rect x=\"8\" y=\"8\" width=\"12\" height=\"12\" rx=\"2\"/><path d=\"M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2\"/>","cut":"<circle cx=\"6.5\" cy=\"17.5\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"17.5\" r=\"2.5\"/><path d=\"M8.3 15.7L18 4M15.7 15.7L6 4\"/>","paste":"<rect x=\"5\" y=\"5\" width=\"14\" height=\"16\" rx=\"2\"/><path d=\"M9 5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1M9 11h6M9 15h4\"/>","rename":"<path d=\"M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3\"/>","trash":"<path d=\"M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2\"/>","undo":"<path d=\"M9 14l-5-5 5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11\"/>","redo":"<path d=\"M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13\"/>","plus":"<path d=\"M12 5v14M5 12h14\"/>","minus":"<path d=\"M5 12h14\"/>","close":"<path d=\"M6 6l12 12M18 6L6 18\"/>","check":"<path d=\"M5 12.5l4.5 4.5L19 7.5\"/>","folder":"<path d=\"M3.5 7a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z\"/>","folder-plus":"<path d=\"M3.5 7a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z\"/><path d=\"M12 10.5v5M9.5 13h5\"/>","file":"<path d=\"M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM14 3.5v5h5\"/>","file-plus":"<path d=\"M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM14 3.5v5h5\"/><path d=\"M12 11.5v6M9 14.5h6\"/>","download":"<path d=\"M12 4v11M7 10l5 5 5-5M5 20h14\"/>","upload":"<path d=\"M12 16V5M7 10l5-5 5 5M5 20h14\"/>","move":"<path d=\"M14 6l5 5-5 5M19 11h-8a6 6 0 0 0-6 6\"/>","swap":"<path d=\"M4 8h15M15 4l4 4-4 4M20 16H5M9 12l-4 4 4 4\"/>","merge":"<path d=\"M12 4v16M3 12h6M6.5 9.5L9 12l-2.5 2.5M21 12h-6M17.5 9.5L15 12l2.5 2.5\"/>","compress":"<path d=\"M4 4l5 5M9 5v4H5M20 4l-5 5M15 5v4h4M4 20l5-5M9 19v-4H5M20 20l-5-5M15 19v-4h4\"/>","external":"<path d=\"M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4\"/>","link":"<path d=\"M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1\"/>","share":"<circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/>","send":"<path d=\"M21 3L10 14M21 3l-6.5 18-4.5-7-7-4.5z\"/>","sync":"<path d=\"M20 11a8 8 0 0 0-14.3-4.3L4 8.5M4 4v4.5h4.5M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8M20 20v-4.5h-4.5\"/>","refresh":"<path d=\"M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5\"/>","cloud":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5z\"/>","cloud-upload":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15\"/><path d=\"M12 20v-7M9.5 15.5L12 13l2.5 2.5M9 18H7\"/>","cloud-download":"<path d=\"M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15M9 18H7\"/><path d=\"M12 12v8M9.5 17.5L12 20l2.5-2.5\"/>","star":"<path d=\"M12 3.5l2.6 5.3 5.9.9-4.25 4.1 1 5.8L12 16.9l-5.25 2.7 1-5.8L3.5 9.7l5.9-.9z\"/>","tag":"<path d=\"M3.5 12.3V4.5a1 1 0 0 1 1-1h7.8a2 2 0 0 1 1.4.6l7.2 7.2a2 2 0 0 1 0 2.8l-6.6 6.6a2 2 0 0 1-2.8 0l-7.2-7.2a2 2 0 0 1-.6-1.4z\"/><path d=\"M8 8h.01\" stroke-width=\"3\"/>","pin":"<path d=\"M9 4h6l-1 5 3 3v2H7v-2l3-3zM12 14v6\"/>","info":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 11v5M12 7.5h.01\"/>","alert":"<path d=\"M12 3.5l9.5 16.5h-19zM12 10v4M12 17h.01\"/>","error":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7.5v5M12 16h.01\"/>","clock":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7v5l3 2\"/>","lock":"<rect x=\"5\" y=\"10.5\" width=\"14\" height=\"10\" rx=\"2\"/><path d=\"M8 10.5V7.5a4 4 0 0 1 8 0v3M12 14.5v2\"/>","unlock":"<rect x=\"5\" y=\"10.5\" width=\"14\" height=\"10\" rx=\"2\"/><path d=\"M8 10.5V7.5a4 4 0 0 1 7.6-1.7M12 14.5v2\"/>","key":"<circle cx=\"8\" cy=\"15\" r=\"4.5\"/><path d=\"M11.2 11.8L20 3M16.5 6.5l2.5 2.5M14 9l2 2\"/>","shield":"<path d=\"M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z\"/><path d=\"M8.5 12l2.5 2.5 4.5-4.5\"/>","heart":"<path d=\"M12 20s-7.5-4.6-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.4-7.5 10-7.5 10z\"/>","user":"<circle cx=\"12\" cy=\"8\" r=\"3.5\"/><path d=\"M5 20a7 7 0 0 1 14 0\"/>","users":"<circle cx=\"9\" cy=\"8.5\" r=\"3\"/><path d=\"M3.5 19.5a5.5 5.5 0 0 1 11 0M15.5 5.8a3 3 0 0 1 0 5.4M17 14.2a5.5 5.5 0 0 1 3.5 5.3\"/>","settings":"<path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/>","terminal":"<path d=\"M5 7l5 5-5 5M12 18h7\"/>","command":"<path d=\"M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3z\"/>","keyboard":"<rect x=\"2.5\" y=\"6\" width=\"19\" height=\"12\" rx=\"2\"/><path d=\"M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M8 14h8\"/>","drive":"<rect x=\"3\" y=\"6\" width=\"18\" height=\"12\" rx=\"2\"/><path d=\"M3 13h18M16.5 15.5h.01M13.5 15.5h.01\"/>","usb":"<path d=\"M8 10h8v9a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2zM9.5 10V3.5h5V10M11 6.5h.01M13 6.5h.01\"/>","sd-card":"<path d=\"M8 3h8.5L19 5.5V20a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6zM9 7v3M12 7v3M15 7v3\"/>","phone":"<rect x=\"6.5\" y=\"2.5\" width=\"11\" height=\"19\" rx=\"2.5\"/><path d=\"M10.5 18.5h3\"/>","phone-ring":"<rect x=\"7.5\" y=\"3.5\" width=\"9\" height=\"17\" rx=\"2\"/><path d=\"M11 17.5h2M4 8.5a6 6 0 0 0 0 7M20 8.5a6 6 0 0 1 0 7\"/>","battery":"<rect x=\"2.5\" y=\"7\" width=\"16.5\" height=\"10\" rx=\"2\"/><path d=\"M21.5 10.5v3M6 10.5v3M9.5 10.5v3\"/>","bolt":"<path d=\"M13 2.5L5 13.5h6l-1 8 8-11h-6z\"/>","wifi":"<path d=\"M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.8 16a4.8 4.8 0 0 1 6.4 0M12 19.5h.01\"/>","bell":"<path d=\"M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15zM10 21h4\"/>","bell-off":"<path d=\"M8.5 5.6A6 6 0 0 1 18 11v4M6 11v5.5l-1.5 2h12.5M10 21h4M3.5 3.5l17 17\"/>","message":"<path d=\"M4 5.5h16a1 1 0 0 1 1 1V16a1 1 0 0 1-1 1h-9l-5 4v-4H4a1 1 0 0 1-1-1V6.5a1 1 0 0 1 1-1zM7.5 10h9M7.5 13h5\"/>","clipboard":"<rect x=\"5\" y=\"4.5\" width=\"14\" height=\"16.5\" rx=\"2\"/><rect x=\"9\" y=\"2.5\" width=\"6\" height=\"4\" rx=\"1\"/><path d=\"M8.5 11h7M8.5 14.5h5\"/>","unlink":"<path d=\"M9.5 6.5l1.3-1.3a4 4 0 0 1 5.7 5.7l-1.3 1.3M14.5 17.5l-1.3 1.3a4 4 0 0 1-5.7-5.7l1.3-1.3M3.5 3.5l17 17\"/>","qr":"<rect x=\"3.5\" y=\"3.5\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"13.5\" y=\"3.5\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"3.5\" y=\"13.5\" width=\"7\" height=\"7\" rx=\"1\"/><path d=\"M13.5 13.5h3v3M20.5 13.5v.01M16.5 20.5h4v-4\"/>","network":"<rect x=\"9\" y=\"3\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"3\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/><rect x=\"15\" y=\"16\" width=\"6\" height=\"5\" rx=\"1\"/><path d=\"M12 8v4M6 16v-2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2\"/>","server":"<rect x=\"3.5\" y=\"4\" width=\"17\" height=\"7\" rx=\"1.5\"/><rect x=\"3.5\" y=\"13\" width=\"17\" height=\"7\" rx=\"1.5\"/><path d=\"M7 7.5h.01M7 16.5h.01\"/>","image":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><circle cx=\"8.5\" cy=\"9.5\" r=\"1.5\"/><path d=\"M21 16l-5-5-9 9\"/>","music":"<path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/>","video":"<rect x=\"3\" y=\"5\" width=\"13\" height=\"14\" rx=\"2\"/><path d=\"M16 10l5-3v10l-5-3\"/>","play":"<path d=\"M7 4.5v15l12-7.5z\"/>","pause":"<path d=\"M9 5v14M15 5v14\"/>","cancel":"<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M9 9l6 6M15 9l-6 6\"/>","crop":"<path d=\"M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14\"/>","rotate":"<path d=\"M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5\"/>","archive":"<rect x=\"3\" y=\"4\" width=\"18\" height=\"5\" rx=\"1\"/><path d=\"M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4\"/>","code":"<path d=\"M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4.5l-3 15\"/>"},"color":{"folder":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/>","folder-open":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M9.3 18h35.2a2 2 0 0 1 1.9 2.7l-5.5 16.5A4 4 0 0 1 37.1 40H6a2 2 0 0 1-1.9-2.7l5.3-17.4A2 2 0 0 1 9.3 18z\"/>","folder-plus":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M5 12h14\"/></g>","folder-minus":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12h14\"/></g>","folder-up":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 19V5M6 11l6-6 6 6\"/></g>","folder-download":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 4v11M7 10l5 5 5-5M5 20h14\"/></g>","folder-link":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1\"/></g>","folder-heart":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"var(--icon-folder-glyph)\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 20s-7.5-4.6-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.4-7.5 10-7.5 10z\"/></g>","folder-lock":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><rect x=\"18\" y=\"26\" width=\"12\" height=\"9.5\" rx=\"2\" fill=\"var(--icon-folder-glyph)\"/><path d=\"M20.5 26v-2.5a3.5 3.5 0 0 1 7 0V26\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"2.4\"/>","folder-clock":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7v5l3 2\"/></g>","folder-move":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M14 6l5 5-5 5M19 11h-8a6 6 0 0 0-6 6\"/></g>","folder-share":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/></g>","folder-users":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"9\" cy=\"8.5\" r=\"3\"/><path d=\"M3.5 19.5a5.5 5.5 0 0 1 11 0M15.5 5.8a3 3 0 0 1 0 5.4M17 14.2a5.5 5.5 0 0 1 3.5 5.3\"/></g>","folder-user":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"8\" r=\"3.5\"/><path d=\"M5 20a7 7 0 0 1 14 0\"/></g>","folder-settings":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","folder-search":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"11\" cy=\"11\" r=\"6.5\"/><path d=\"M16 16l4.5 4.5\"/></g>","folder-image":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><circle cx=\"8.5\" cy=\"9.5\" r=\"1.5\"/><path d=\"M21 16l-5-5-9 9\"/></g>","folder-music":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><g transform=\"translate(16 20) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-folder-glyph)\" stroke-width=\"3.90\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/></g>","folder-video":"<path fill=\"var(--icon-folder-back)\" d=\"M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-folder)\" d=\"M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z\"/><path d=\"M21 23.5v11l9-5.5z\" fill=\"var(--icon-folder-glyph)\"/>","home":"<path fill=\"var(--icon-blue)\" d=\"M24 5.5l18 15.2V40a3 3 0 0 1-3 3H29.5V31h-11v12H9a3 3 0 0 1-3-3V20.7z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M24 5.5l18 15.2v3.1L24 8.6 6 23.8v-3.1z\"/>","trash":"<rect x=\"15\" y=\"4\" width=\"18\" height=\"6\" rx=\"2\" fill=\"var(--icon-slate)\"/><rect x=\"7\" y=\"8\" width=\"34\" height=\"6\" rx=\"2\" fill=\"var(--icon-slate)\"/><path fill=\"var(--icon-paper-fold)\" d=\"M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z\"/><path d=\"M20 22v14M28 22v14\" stroke=\"var(--icon-slate)\" stroke-width=\"3\" stroke-linecap=\"round\"/>","trash-full":"<rect x=\"15\" y=\"4\" width=\"18\" height=\"6\" rx=\"2\" fill=\"var(--icon-red)\"/><rect x=\"7\" y=\"8\" width=\"34\" height=\"6\" rx=\"2\" fill=\"var(--icon-red)\"/><path fill=\"var(--icon-red)\" d=\"M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z\"/><path d=\"M20 22v14M28 22v14\" stroke=\"var(--icon-on)\" stroke-width=\"3\" stroke-linecap=\"round\" opacity=\"0.9\"/>","drive":"<path fill=\"var(--icon-slate)\" d=\"M11.5 9h25a3 3 0 0 1 2.9 2.3L43 27H5l3.6-15.7A3 3 0 0 1 11.5 9z\"/><rect x=\"5\" y=\"26\" width=\"38\" height=\"13\" rx=\"3\" fill=\"var(--icon-slate-deep)\"/><circle cx=\"36\" cy=\"32.5\" r=\"2\" fill=\"var(--icon-on)\"/><circle cx=\"30\" cy=\"32.5\" r=\"2\" fill=\"var(--icon-on)\" opacity=\"0.7\"/>","drive-external":"<rect x=\"5\" y=\"12\" width=\"38\" height=\"24\" rx=\"5\" fill=\"var(--icon-slate-deep)\"/><rect x=\"5\" y=\"12\" width=\"38\" height=\"12\" rx=\"5\" fill=\"var(--icon-slate)\"/><rect x=\"5\" y=\"20\" width=\"38\" height=\"4\" fill=\"var(--icon-slate)\"/><circle cx=\"36\" cy=\"30\" r=\"2\" fill=\"var(--icon-on)\"/>","server":"<rect x=\"6\" y=\"6\" width=\"36\" height=\"16\" rx=\"3\" fill=\"var(--icon-slate)\"/><rect x=\"6\" y=\"26\" width=\"36\" height=\"16\" rx=\"3\" fill=\"var(--icon-slate-deep)\"/><circle cx=\"34\" cy=\"14\" r=\"2\" fill=\"var(--icon-on)\"/><circle cx=\"34\" cy=\"34\" r=\"2\" fill=\"var(--icon-on)\"/><rect x=\"12\" y=\"12.7\" width=\"12\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\" opacity=\"0.6\"/>","usb":"<rect x=\"16\" y=\"4\" width=\"16\" height=\"14\" rx=\"2\" fill=\"var(--icon-paper-fold)\"/><rect x=\"20\" y=\"8\" width=\"3\" height=\"4\" rx=\"1\" fill=\"var(--icon-slate)\"/><rect x=\"25\" y=\"8\" width=\"3\" height=\"4\" rx=\"1\" fill=\"var(--icon-slate)\"/><rect x=\"12\" y=\"16\" width=\"24\" height=\"28\" rx=\"5\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"34\" width=\"8\" height=\"3\" rx=\"1.5\" fill=\"var(--icon-paper-fold)\"/>","sd-card":"<path fill=\"var(--icon-slate-deep)\" d=\"M15 4h19l6 6v30a4 4 0 0 1-4 4H12a4 4 0 0 1-4-4V11z\"/><rect x=\"16\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"21\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"26\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/><rect x=\"31\" y=\"9\" width=\"3.4\" height=\"10\" rx=\"1\" fill=\"var(--icon-yellow)\"/>","phone":"<rect x=\"12\" y=\"3\" width=\"24\" height=\"42\" rx=\"6\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"7\" width=\"18\" height=\"31\" rx=\"2.5\" fill=\"var(--icon-on)\" opacity=\"0.92\"/><rect x=\"20\" y=\"40.2\" width=\"8\" height=\"2.2\" rx=\"1.1\" fill=\"var(--icon-on)\"/>","network":"<rect x=\"17\" y=\"4\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><rect x=\"5\" y=\"33\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><rect x=\"29\" y=\"33\" width=\"14\" height=\"11\" rx=\"2\" fill=\"var(--icon-blue)\"/><path d=\"M24 15v8M12 33v-4a3 3 0 0 1 3-3h18a3 3 0 0 1 3 3v4\" fill=\"none\" stroke=\"var(--icon-blue-deep)\" stroke-width=\"3\" stroke-linecap=\"round\"/>","cloud":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/>","cloud-upload":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/><g transform=\"translate(16 17) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.50\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 19V5M6 11l6-6 6 6\"/></g>","cloud-download":"<path fill=\"var(--icon-blue)\" d=\"M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z\"/><g transform=\"translate(16 18) scale(0.6667)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.50\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M6 13l6 6 6-6\"/></g>","file":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-paper-fold)\"/>","file-text":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-blue)\"/>","file-doc":"<path fill=\"var(--icon-blue)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"27\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"32\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/>","file-sheet":"<path fill=\"var(--icon-green)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"21\" width=\"19\" height=\"15\" rx=\"1.5\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/><path d=\"M15 28.5h19M24.5 21v15\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/>","file-slides":"<path fill=\"var(--icon-orange)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"22\" width=\"19\" height=\"12\" rx=\"1.5\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2.6\"/><rect x=\"18.5\" y=\"26.5\" width=\"12\" height=\"3\" rx=\"1\" fill=\"var(--icon-on)\"/>","file-pdf":"<path fill=\"var(--icon-red)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"19\" width=\"12\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"15\" y=\"24\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-on)\"/><rect x=\"13\" y=\"30\" width=\"23\" height=\"9\" rx=\"2\" fill=\"var(--icon-on)\"/><path d=\"M16.5 37v-5h2a1.5 1.5 0 0 1 0 3h-2M22.5 37v-5h1.5a2.5 2.5 0 0 1 0 5zM29.5 37v-5h3M29.5 34.5h2.4\" fill=\"none\" stroke=\"var(--icon-red)\" stroke-width=\"1.3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>","file-code":"<path fill=\"var(--icon-purple)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-on)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(14.5 20) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"3.36\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4.5l-3 15\"/></g>","file-config":"<path fill=\"var(--icon-slate)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(15 20) scale(0.7500)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"3.47\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","file-font":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M14.5 34l5-14h3l5 14h-3.1l-1.1-3.3h-4.6L17.6 34zm5-5.9h3l-1.5-4.6z\"/><path fill=\"var(--icon-slate)\" d=\"M31.6 34.2c-2 0-3.3-1.2-3.3-3 0-2 1.5-3 4.3-3.1l1.9-.1v-.4c0-.9-.6-1.4-1.7-1.4-1 0-1.6.4-1.8 1.1h-2.6c.2-2.1 2-3.4 4.5-3.4 2.7 0 4.3 1.3 4.3 3.6V34h-2.6v-1.3c-.5.9-1.6 1.5-3 1.5zm.9-2.1c1.1 0 2-.7 2-1.7v-.6l-1.6.1c-1 .1-1.5.5-1.5 1.1s.4 1.1 1.1 1.1z\"/>","file-type":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M16 20h17v3.4h-6.8V36h-3.4V23.4H16z\"/>","file-lines":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"15\" y=\"20\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"25\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"30\" width=\"19\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><rect x=\"15\" y=\"35\" width=\"13\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/>","file-list":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"16.5\" cy=\"22.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"21\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><circle cx=\"16.5\" cy=\"27.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"26\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/><circle cx=\"16.5\" cy=\"32.3\" r=\"1.6\" fill=\"var(--icon-slate)\"/><rect x=\"20\" y=\"31\" width=\"14\" height=\"2.6\" rx=\"1.3\" fill=\"var(--icon-slate)\"/>","file-script":"<path fill=\"var(--icon-slate-deep)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-slate)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><g transform=\"translate(13 21) scale(0.5833)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.14\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 6l6 6-6 6\"/></g><rect x=\"25\" y=\"32\" width=\"9\" height=\"2.8\" rx=\"1.4\" fill=\"var(--icon-on)\"/>","file-image":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-green)\" opacity=\"0.10\"/><circle cx=\"19\" cy=\"23\" r=\"3\" fill=\"var(--icon-orange)\"/><path fill=\"var(--icon-green)\" d=\"M13 38l7.5-9 4.5 5 4-4.5 7 8.5z\"/>","file-video":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-red)\" opacity=\"0.10\"/><rect x=\"14\" y=\"21\" width=\"21\" height=\"15\" rx=\"2.5\" fill=\"var(--icon-red)\"/><path d=\"M22 24.8v7.4l6-3.7z\" fill=\"var(--icon-on)\"/>","file-audio":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-purple)\" opacity=\"0.10\"/><g transform=\"translate(14 19) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"3.36\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 18V5.5l11-2V16\"/><circle cx=\"6.5\" cy=\"18\" r=\"2.5\"/><circle cx=\"17.5\" cy=\"16\" r=\"2.5\"/></g>","file-vector":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"9\" y=\"4\" width=\"31\" height=\"40\" rx=\"3\" fill=\"var(--icon-purple)\" opacity=\"0.10\"/><path fill=\"var(--icon-purple)\" d=\"M13 38l7.5-10 4.5 6 3-4 6 8z\"/><g transform=\"translate(23 17) scale(0.5833)\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"4.46\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3\"/></g>","file-archive":"<path fill=\"var(--icon-folder)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-folder-back)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"24\" y=\"6.0\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"9.4\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"12.8\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"16.2\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"19.6\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"23.0\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"24\" y=\"26.4\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"21\" y=\"29.8\" width=\"3\" height=\"3\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"20.5\" y=\"33\" width=\"7\" height=\"6\" rx=\"1.5\" fill=\"var(--icon-folder-glyph)\"/>","file-archive-alt":"<path fill=\"var(--icon-slate)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-slate-deep)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><rect x=\"24\" y=\"6.0\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"9.4\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"12.8\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"16.2\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"19.6\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"23.0\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"26.4\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"29.8\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"24\" y=\"33.2\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"21\" y=\"36.599999999999994\" width=\"3\" height=\"3\" fill=\"var(--icon-slate-deep)\"/><rect x=\"20.5\" y=\"39\" width=\"7\" height=\"3\" rx=\"1\" fill=\"var(--icon-slate-deep)\"/>","file-package":"<rect x=\"6\" y=\"7\" width=\"36\" height=\"11\" rx=\"3\" fill=\"var(--icon-red)\"/><rect x=\"6\" y=\"18\" width=\"36\" height=\"11\" rx=\"0\" fill=\"var(--icon-blue)\"/><rect x=\"6\" y=\"29\" width=\"36\" height=\"12\" rx=\"3\" fill=\"var(--icon-green)\"/><rect x=\"21\" y=\"7\" width=\"7\" height=\"34\" fill=\"var(--icon-folder)\"/><rect x=\"19.5\" y=\"20.5\" width=\"10\" height=\"7\" rx=\"1.5\" fill=\"var(--icon-folder-glyph)\"/><rect x=\"36\" y=\"10\" width=\"2.4\" height=\"5\" rx=\"1\" fill=\"var(--icon-on)\" opacity=\"0.8\"/>","file-verified":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-blue)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><g transform=\"translate(30 30) scale(0.5000)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.20\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12.5l4.5 4.5L19 7.5\"/></g>","file-error":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-red)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><path d=\"M36 31v6\" stroke=\"var(--icon-on)\" stroke-width=\"2.8\" stroke-linecap=\"round\"/><circle cx=\"36\" cy=\"40.6\" r=\"1.6\" fill=\"var(--icon-on)\"/>","file-new":"<path fill=\"var(--icon-paper)\" d=\"M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z\"/><path fill=\"var(--icon-paper-fold)\" d=\"M29 4l11 11h-8a3 3 0 0 1-3-3z\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"var(--icon-blue)\"/><circle cx=\"36\" cy=\"36\" r=\"9\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"2\"/><g transform=\"translate(30 30) scale(0.5000)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"5.20\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14M5 12h14\"/></g>","lock":"<path d=\"M16 21v-5a8 8 0 0 1 16 0v5\" fill=\"none\" stroke=\"var(--icon-purple)\" stroke-width=\"4.5\"/><rect x=\"10\" y=\"20\" width=\"28\" height=\"23\" rx=\"5\" fill=\"var(--icon-purple)\"/><circle cx=\"24\" cy=\"30\" r=\"3\" fill=\"var(--icon-on)\"/><rect x=\"22.6\" y=\"31\" width=\"2.8\" height=\"6\" rx=\"1.4\" fill=\"var(--icon-on)\"/>","shield":"<path fill=\"var(--icon-green)\" d=\"M24 4l16 6v11.5C40 31.5 33.3 39.6 24 44 14.7 39.6 8 31.5 8 21.5V10z\"/><g transform=\"translate(14 13) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.08\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M5 12.5l4.5 4.5L19 7.5\"/></g>","key":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"8\" cy=\"15\" r=\"4.5\"/><path d=\"M11.2 11.8L20 3M16.5 6.5l2.5 2.5M14 9l2 2\"/></g>","eye":"<path fill=\"var(--icon-blue)\" d=\"M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z\"/><circle cx=\"24\" cy=\"24\" r=\"7.5\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"24\" r=\"4\" fill=\"var(--icon-blue-deep)\"/>","eye-off":"<path fill=\"var(--icon-paper-fold)\" d=\"M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z\"/><circle cx=\"24\" cy=\"24\" r=\"7.5\" fill=\"var(--icon-slate)\"/><path d=\"M9 9l30 30\" stroke=\"var(--icon-slate)\" stroke-width=\"4.5\" stroke-linecap=\"round\"/>","share":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-blue)\" stroke-width=\"2.51\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"18\" cy=\"5.5\" r=\"2.5\"/><circle cx=\"6\" cy=\"12\" r=\"2.5\"/><circle cx=\"18\" cy=\"18.5\" r=\"2.5\"/><path d=\"M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1\"/></g>","send":"<path fill=\"var(--icon-blue)\" d=\"M43 5L4.5 20.5l14 6.5 6.5 14z\"/><path fill=\"var(--icon-blue-deep)\" d=\"M43 5L18.5 27 25 41z\"/>","sync":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-green)\" stroke-width=\"2.51\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M20 11a8 8 0 0 0-14.3-4.3L4 8.5M4 4v4.5h4.5M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8M20 20v-4.5h-4.5\"/></g>","history":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5M3.5 4v4.5H8M12 8v4.5l3 2\"/></g>","star":"<path fill=\"var(--icon-orange)\" d=\"M24 4.5l5.8 11.8 13 1.9-9.4 9.2 2.2 12.9L24 34.2l-11.6 6.1 2.2-12.9-9.4-9.2 13-1.9z\"/>","tag":"<path fill=\"var(--icon-blue)\" d=\"M5 22.3V8a3 3 0 0 1 3-3h14.3a4 4 0 0 1 2.8 1.2l16.2 16.2a4 4 0 0 1 0 5.6L27.6 41.7a4 4 0 0 1-5.6 0L6.2 25.1A4 4 0 0 1 5 22.3z\"/><circle cx=\"14.5\" cy=\"14.5\" r=\"3.5\" fill=\"var(--icon-on)\"/>","info":"<circle cx=\"24\" cy=\"24\" r=\"20\" fill=\"var(--icon-blue)\"/><rect x=\"21.5\" y=\"21\" width=\"5\" height=\"14\" rx=\"2.5\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"14.5\" r=\"3\" fill=\"var(--icon-on)\"/>","settings":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.37 5.19 L10.78 2.88 L13.22 2.88 L13.63 5.19 L15.66 6.03 L17.58 4.69 L19.31 6.42 L17.97 8.34 L18.81 10.37 L21.12 10.78 L21.12 13.22 L18.81 13.63 L17.97 15.66 L19.31 17.58 L17.58 19.31 L15.66 17.97 L13.63 18.81 L13.22 21.12 L10.78 21.12 L10.37 18.81 L8.34 17.97 L6.42 19.31 L4.69 17.58 L6.03 15.66 L5.19 13.63 L2.88 13.22 L2.88 10.78 L5.19 10.37 L6.03 8.34 L4.69 6.42 L6.42 4.69 L8.34 6.03Z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></g>","sliders":"<g transform=\"translate(3 3) scale(1.7500)\" fill=\"none\" stroke=\"var(--icon-slate)\" stroke-width=\"2.40\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M4 7h9M17 7h3M4 17h3M11 17h9\"/><circle cx=\"15\" cy=\"7\" r=\"2\"/><circle cx=\"9\" cy=\"17\" r=\"2\"/></g>","terminal":"<rect x=\"4\" y=\"6\" width=\"40\" height=\"36\" rx=\"6\" fill=\"var(--icon-slate-deep)\"/><g transform=\"translate(9 14) scale(0.8333)\" fill=\"none\" stroke=\"var(--icon-on)\" stroke-width=\"4.08\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 6l6 6-6 6\"/></g><rect x=\"25\" y=\"29\" width=\"11\" height=\"3.4\" rx=\"1.7\" fill=\"var(--icon-on)\"/>","alert":"<path fill=\"var(--icon-orange)\" d=\"M21.4 6.5a3 3 0 0 1 5.2 0l17 29.5a3 3 0 0 1-2.6 4.5H7a3 3 0 0 1-2.6-4.5z\"/><rect x=\"21.6\" y=\"16\" width=\"4.8\" height=\"13\" rx=\"2.4\" fill=\"var(--icon-on)\"/><circle cx=\"24\" cy=\"34\" r=\"2.6\" fill=\"var(--icon-on)\"/>"}};

  // Glyphs EchoConnect needs that EchoFiles doesn't (24-unit grid, 2px stroke, round caps),
  // drawn to match assets/icons/glyph/. Merged into the shared EchoFiles set.
  var CONNECT_GLYPHS = {
    "bluetooth": "<path d=\"M7 7.5l10 9-5 4.5V3l5 4.5-10 9\"/>",
    "laptop": "<rect x=\"4.5\" y=\"4.5\" width=\"15\" height=\"11\" rx=\"1.5\"/><path d=\"M2.5 19.5h19\"/>",
    "camera": "<path d=\"M4 7.5h3.5L9 5h6l1.5 2.5H20a1 1 0 0 1 1 1V18a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8.5a1 1 0 0 1 1-1z\"/><circle cx=\"12\" cy=\"13\" r=\"3.5\"/>",
    "mic": "<rect x=\"9\" y=\"3\" width=\"6\" height=\"11\" rx=\"3\"/><path d=\"M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21\"/>",
    "speaker": "<path d=\"M4 9.5h3.5L12 5.5v13l-4.5-4H4z\"/><path d=\"M15.5 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11\"/>",
    "call": "<path d=\"M5 4h3.5l1.5 4.5-2.2 1.4a11 11 0 0 0 6.3 6.3l1.4-2.2 4.5 1.5V19a1.5 1.5 0 0 1-1.5 1.5A16.5 16.5 0 0 1 3.5 5.5 1.5 1.5 0 0 1 5 4z\"/>",
    "scan": "<path d=\"M3.5 8V5a1.5 1.5 0 0 1 1.5-1.5h3M16 3.5h3A1.5 1.5 0 0 1 20.5 5v3M20.5 16v3a1.5 1.5 0 0 1-1.5 1.5h-3M8 20.5H5A1.5 1.5 0 0 1 3.5 19v-3M7 12h10\"/>",
    "moon": "<path d=\"M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z\"/>",
    "cursor-text": "<path d=\"M9 4h6M9 20h6M12 4v16\"/>",
    "flashlight": "<path d=\"M8 3h8v4l-2 4v10h-4V11L8 7zM8 7h8M12 14v2\"/>"
  };
  for (var gk in CONNECT_GLYPHS) ICONS.glyphs[gk] = CONNECT_GLYPHS[gk];
(function () {
  var React = window.React;
  var h = React.createElement;
  var useState = React.useState;

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
  var SYS_SANS = "Inter, 'SamsungOne', 'Google Sans', Roboto, system-ui, sans-serif";
  var LAPTOP = "Aditya's laptop";

  // ------------------------------------------------------------ icons (shared with EchoFiles)
  function Icon(p) {
    var size = p.size || 20;
    return h("svg", {
      className: cx("ec-icon", p.className), width: size, height: size, viewBox: "0 0 24 24",
      fill: "none", stroke: "currentColor", strokeWidth: p.strokeWidth || 2, strokeLinecap: "round",
      strokeLinejoin: "round", "aria-hidden": p.label ? undefined : "true", role: p.label ? "img" : undefined,
      "aria-label": p.label, style: p.style, dangerouslySetInnerHTML: { __html: ICONS.glyphs[p.name] || "" }
    });
  }

  var EXT = {
    jpg: "file-image", jpeg: "file-image", png: "file-image", webp: "file-image", heic: "file-image",
    mp4: "file-video", mkv: "file-video", mov: "file-video", mp3: "file-audio", m4a: "file-audio", opus: "file-audio",
    pdf: "file-pdf", doc: "file-doc", docx: "file-doc", txt: "file-lines", md: "file-text",
    xls: "file-sheet", xlsx: "file-sheet", csv: "file-sheet", ppt: "file-slides", pptx: "file-slides",
    zip: "file-archive", apk: "file-package", rs: "file-code", py: "file-code", sh: "file-script"
  };
  function iconFor(name, kind) {
    if (kind === "folder") return "folder";
    var m = /\.([^.]+)$/.exec(name || "");
    return (m && EXT[m[1].toLowerCase()]) || "file";
  }
  function FileIcon(p) {
    var size = p.size || 32;
    return h("span", { className: cx("ec-ficon", p.className), style: { width: size, height: size }, "aria-hidden": "true" },
      h("svg", { viewBox: "0 0 48 48", dangerouslySetInnerHTML: { __html: ICONS.color[p.name] || ICONS.color.file } }));
  }

  // ------------------------------------------------------------ actions
  function Button(p) {
    var o = rest(p, ["variant", "size", "icon", "block", "className", "children"]);
    o.className = cx("ec-btn", p.variant && "ec-btn-" + p.variant, p.size && "ec-btn-" + p.size, p.block && "ec-btn-block", p.className);
    o.type = o.type || "button";
    return h("button", o, p.icon ? h(Icon, { name: p.icon, size: p.size === "sm" ? 16 : 18 }) : null, p.children);
  }

  function IconButton(p) {
    var o = rest(p, ["icon", "label", "pressed", "className", "size"]);
    o.className = cx("ec-ibtn", p.className);
    o.type = "button";
    o["aria-label"] = p.label;
    o.title = p.label;
    if (p.pressed !== undefined) o["aria-pressed"] = String(!!p.pressed);
    return h("button", o, h(Icon, { name: p.icon, size: p.size || 22 }));
  }

  function Switch(p) {
    var st = useState(!!p.defaultChecked);
    var on = p.checked !== undefined ? p.checked : st[0];
    return h("button", {
      type: "button", role: "switch", "aria-checked": String(on), className: "ec-switch", id: p.id, disabled: p.disabled,
      "aria-label": p.label, onClick: function (e) { e.stopPropagation(); st[1](!on); p.onChange && p.onChange(!on); }
    });
  }

  function Checkbox(p) {
    var st = useState(!!p.defaultChecked);
    var val = p.checked !== undefined ? p.checked : st[0];
    return h("label", { className: "ec-check-row" },
      h("button", {
        type: "button", role: "checkbox", "aria-checked": String(!!val), className: "ec-check", id: p.id,
        onClick: function () { st[1](!val); p.onChange && p.onChange(!val); }
      }, h(Icon, { name: "check", size: 14, strokeWidth: 3 })),
      p.children ? h("span", null, p.children) : null);
  }

  function SegmentedControl(p) {
    var st = useState(p.value !== undefined ? p.value : (p.options[0] || {}).value);
    var v = p.onChange && p.value !== undefined ? p.value : st[0];
    return h("div", { className: cx("ec-seg", p.block && "ec-seg-block"), role: "group", "aria-label": p.label },
      p.options.map(function (o) {
        return h("button", {
          key: o.value, type: "button", "aria-pressed": String(o.value === v), "aria-label": o.label,
          onClick: function () { st[1](o.value); p.onChange && p.onChange(o.value); }
        }, o.icon ? h(Icon, { name: o.icon, size: 16 }) : null, o.text || null);
      }));
  }

  // ------------------------------------------------------------ inputs
  function TextField(p) {
    var o = rest(p, ["label", "icon", "error", "hint", "className", "id", "trailing"]);
    return h("div", { className: cx("ec-fieldwrap", p.className) },
      p.label ? h("label", { className: "ec-field-label", htmlFor: p.id }, p.label) : null,
      h("div", { className: cx("ec-field", p.error && "ec-field-invalid") },
        p.icon ? h(Icon, { name: p.icon, size: 18 }) : null,
        h("input", Object.assign({ id: p.id, "aria-invalid": p.error ? "true" : undefined }, o)),
        p.trailing || null),
      p.error ? h("div", { className: "ec-field-msg", role: "alert" }, p.error) : p.hint ? h("div", { className: "ec-field-hint" }, p.hint) : null);
  }

  // ------------------------------------------------------------ feedback
  function StatePill(p) {
    return h("span", { className: cx("ec-pill", p.tone && "ec-pill-" + p.tone, p.icon && "ec-pill-icon") },
      p.icon ? h(Icon, { name: p.icon, size: 13, strokeWidth: 2.4 }) : null, p.children);
  }

  function Spinner(p) { return h("span", { className: cx("ec-spinner", p.large && "ec-spinner-lg"), role: "status", "aria-label": p.label || "Working" }); }

  function ProgressBar(p) {
    return h("span", { className: cx("ec-progress", p.done && "ec-progress-done", p.value === undefined && "ec-progress-indet"), role: "progressbar", "aria-valuenow": p.value },
      h("i", { style: { width: (p.value || 0) + "%" } }));
  }

  function Snackbar(p) {
    return h("div", { className: cx("ec-snack", p.tone && "ec-snack-" + p.tone), role: "status" },
      p.icon ? h(Icon, { name: p.icon, size: 18 }) : null,
      h("span", { className: "ec-snack-text" }, p.children),
      p.action ? h("button", { type: "button", className: "ec-snack-action" }, p.action) : null);
  }

  function Banner(p) {
    var icon = p.icon || { warning: "alert", danger: "error", success: "check" }[p.tone] || "info";
    return h("div", { className: cx("ec-banner", p.tone && "ec-banner-" + p.tone) },
      h(Icon, { name: icon, size: 18 }),
      h("div", { className: "ec-banner-text" }, p.title ? h("strong", null, p.title) : null, p.children),
      p.action || null);
  }

  // ------------------------------------------------------------ structure
  function AppBar(p) {
    return h("header", { className: "ec-appbar" },
      p.back ? h(IconButton, { icon: "arrow-left", label: "Back" }) : null,
      h("div", { className: cx("ec-appbar-titles", !p.back && "ec-appbar-titles-flush") },
        h("h1", { className: "ec-appbar-title" }, p.title),
        p.sub ? h("span", { className: "ec-appbar-sub" }, p.sub) : null),
      (p.actions || []).map(function (a, i) { return h(IconButton, { key: i, icon: a.icon, label: a.label, pressed: a.pressed }); }));
  }

  var NAV = [["home", "Home"], ["clipboard", "Clipboard"], ["swap", "Transfers"], ["settings", "Settings"]];
  function BottomNav(p) {
    var st = useState(p.active || 0);
    var cur = p.onChange ? p.active || 0 : st[0];
    var badges = p.badges || {};
    return h("nav", { className: "ec-nav", "aria-label": "Sections" },
      NAV.map(function (it, i) {
        return h("button", {
          key: i, type: "button", className: "ec-nav-item", "aria-current": i === cur ? "page" : undefined,
          onClick: function () { st[1](i); p.onChange && p.onChange(i); }
        },
          h("span", { className: "ec-nav-glyph" }, h(Icon, { name: it[0], size: 22 }), badges[i] ? h("span", { className: "ec-nav-badge" }, badges[i]) : null),
          h("span", { className: "ec-nav-label" }, it[1]));
      }));
  }

  function SectionHeader(p) {
    return h("div", { className: "ec-section" },
      h("h2", { className: "ec-section-label" }, p.children),
      p.action ? h("button", { type: "button", className: "ec-section-action" }, p.action) : null);
  }

  function ListGroup(p) {
    return h("section", { className: "ec-group" },
      p.title ? h(SectionHeader, { action: p.action }, p.title) : null,
      h("div", { className: "ec-groupbox" }, p.children),
      p.foot ? h("p", { className: "ec-group-foot" }, p.foot) : null);
  }

  function ListRow(p) {
    var lead = p.icon ? h("span", { className: cx("ec-row-icon", p.tint && "ec-row-icon-" + p.tint) }, h(Icon, { name: p.icon, size: 20 }))
      : p.file ? h(FileIcon, { name: p.file, size: 36 })
      : p.avatar ? h("span", { className: "ec-avatar", style: { background: "var(--icon-" + (p.avatarColor || "purple") + ")" } }, p.avatar)
      : p.lead || null;
    var Tag = p.onClick || p.chevron ? "button" : "div";
    return h(Tag, { type: Tag === "button" ? "button" : undefined, className: cx("ec-row", p.sub && "ec-row-two", p.disabled && "ec-row-off"), onClick: p.onClick },
      lead,
      h("span", { className: "ec-row-text" },
        h("span", { className: "ec-row-title" }, p.title),
        p.sub ? h("span", { className: "ec-row-sub" }, p.sub) : null,
        p.below || null),
      p.trailing !== undefined ? h("span", { className: "ec-row-trail" }, p.trailing) : null,
      p.chevron ? h(Icon, { name: "chevron-right", size: 18, className: "ec-row-chev" }) : null);
  }

  function BottomSheet(p) {
    return h("div", { className: "ec-sheet", role: "dialog", "aria-label": p.title },
      h("span", { className: "ec-sheet-handle", "aria-hidden": "true" }),
      p.title ? h("div", { className: "ec-sheet-head" }, p.icon ? h(Icon, { name: p.icon, size: 20 }) : null, h("h2", { className: "ec-sheet-title" }, p.title)) : null,
      h("div", { className: "ec-sheet-body" }, p.children),
      p.footer ? h("div", { className: "ec-sheet-foot" }, p.footer) : null);
  }

  function Dialog(p) {
    return h("div", { className: cx("ec-dialog", p.tone === "danger" && "ec-dialog-danger"), role: "dialog", "aria-label": p.title },
      h("h2", { className: "ec-dialog-title" }, p.title),
      h("div", { className: "ec-dialog-body" }, p.children),
      h("div", { className: "ec-dialog-foot" }, p.actions));
  }

  // ------------------------------------------------------------ device
  /** A phone frame for whole screens: flat slate body, Android status bar, gesture bar. */
  function PhoneFrame(p) {
    var w = p.width || 360, hgt = p.height || 760;
    return h("div", { className: "ec-device", style: { width: w + 16 } },
      h("span", { className: "ec-device-key ec-device-vol", "aria-hidden": "true" }),
      h("span", { className: "ec-device-key ec-device-side", "aria-hidden": "true" }),
      h("div", { className: cx("ec", "ec-screen", p.dim && "ec-screen-dim"), style: { height: hgt } },
        h("div", { className: "ec-sysbar" },
          h("span", { className: "ec-sysbar-time" }, p.time || "10:42"),
          h("span", { className: "ec-sysbar-cam", "aria-hidden": "true" }),
          h("span", { className: "ec-sysbar-icons" },
            p.bluetooth !== false ? h(Icon, { name: "bluetooth", size: 13 }) : null,
            h(Icon, { name: "wifi", size: 14 }),
            h("span", { className: "ec-sysbar-batt" }, (p.battery || 72) + "%"))),
        h("div", { className: "ec-screen-body" }, p.children),
        h("div", { className: "ec-gesture", "aria-hidden": "true" }, h("i"))));
  }

  /** The laptop, drawn flat like the colour icons, with EchoFiles open on its screen. */
  function LaptopDevice(p) {
    var w = p.width || 220, s = p.state || "connected";
    var rows = [0, 1, 2, 3, 4, 5, 6];
    var scr = s === "away"
      ? [h("rect", { key: "off", x: 38, y: 14, width: 244, height: 150, fill: "#05060a" })]
      : s === "locked"
        ? [h("rect", { key: "l", x: 38, y: 14, width: 244, height: 150, style: { fill: "var(--bg-deep)" } }),
          h("text", { key: "t", x: 160, y: 84, textAnchor: "middle", fontSize: 26, fontWeight: 800, style: { fill: "var(--ink-strong)", fontFamily: "var(--font-mono)" } }, "10:42"),
          h("rect", { key: "f", x: 120, y: 102, width: 80, height: 14, rx: 2, style: { fill: "var(--bg)", stroke: "var(--accent)" }, strokeWidth: 1.5 }),
          h("text", { key: "u", x: 160, y: 136, textAnchor: "middle", fontSize: 8, style: { fill: "var(--ink-muted)", fontFamily: "var(--font-mono)" } }, "Locked from your phone")]
        : [h("rect", { key: "bg", x: 38, y: 14, width: 244, height: 150, style: { fill: "var(--bg-deep)" } }),
          // the EchoFiles window: 2px accent border, tabs, toolbar, sidebar, rows, status bar
          h("rect", { key: "win", x: 42, y: 18, width: 236, height: 142, style: { fill: "var(--bg)", stroke: "var(--accent)" }, strokeWidth: 1.5 }),
          h("rect", { key: "tabs", x: 43, y: 19, width: 234, height: 9, style: { fill: "var(--bg-deep)" } }),
          h("rect", { key: "tab", x: 43, y: 19, width: 46, height: 9, style: { fill: "var(--bg)" } }),
          h("rect", { key: "tabbar", x: 43, y: 19, width: 46, height: 1.5, style: { fill: "var(--accent)" } }),
          h("rect", { key: "tool", x: 60, y: 31, width: 120, height: 6, rx: 1, style: { fill: "var(--bg-deep)", stroke: "var(--line-strong)" }, strokeWidth: 0.6 }),
          h("rect", { key: "side", x: 43, y: 40, width: 52, height: 113, style: { fill: "var(--bg-sunken)" } }),
          h("g", { key: "marks" }, ["linux", "windows", "phone", "network"].map(function (wn, i) {
            return h("g", { key: wn },
              h("rect", { x: 48, y: 46 + i * 26, width: 3.5, height: 3.5, style: { fill: "var(--world-" + wn + ")" } }),
              h("rect", { x: 54, y: 46 + i * 26, width: 22, height: 3, rx: 1, style: { fill: "var(--ink-faint)" } }),
              h("rect", { x: 48, y: 54 + i * 26, width: 34, height: 3, rx: 1, style: { fill: i === 2 ? "var(--accent-ink)" : "var(--line-strong)" } }),
              h("rect", { x: 48, y: 61 + i * 26, width: 28, height: 3, rx: 1, style: { fill: "var(--line-strong)" } }));
          })),
          h("g", { key: "rows" }, rows.map(function (r) {
            return h("g", { key: r },
              r === 2 ? h("rect", { x: 97, y: 42 + r * 15, width: 179, height: 13, style: { fill: "var(--selection)" } }) : null,
              h("rect", { x: 101, y: 45 + r * 15, width: 9, height: 7, rx: 1, style: { fill: r < 3 ? "var(--icon-folder)" : r === 4 ? "var(--icon-red)" : "var(--icon-blue)" } }),
              h("rect", { x: 115, y: 47 + r * 15, width: 60 + (r * 23) % 50, height: 3, rx: 1, style: { fill: r === 2 ? "var(--ink-strong)" : "var(--ink-muted)" } }),
              h("rect", { x: 236, y: 47 + r * 15, width: 30, height: 3, rx: 1, style: { fill: "var(--ink-faint)" } }));
          })),
          h("rect", { key: "status", x: 43, y: 153, width: 234, height: 6, style: { fill: "var(--bg-deep)" } })];
    return h("div", { className: cx("ec-laptop", "ec-laptop-" + s), style: { width: w }, role: "img", "aria-label": LAPTOP + (s === "away" ? ", not nearby" : s === "locked" ? ", locked" : ", EchoFiles open") },
      h("svg", { viewBox: "0 0 320 196", width: "100%", style: { display: "block" } },
        h("rect", { x: 30, y: 6, width: 260, height: 166, rx: 10, style: { fill: "var(--icon-slate-deep)" } }),
        scr,
        h("circle", { cx: 160, cy: 10, r: 1.6, fill: "#000" }),
        h("path", { d: "M8 174h304l-6 14a6 6 0 0 1-5.5 4H19.5a6 6 0 0 1-5.5-4z", style: { fill: "var(--icon-slate)" } }),
        h("rect", { x: 136, y: 174, width: 48, height: 4, rx: 2, style: { fill: "var(--icon-slate-deep)" } })));
  }

  /** A small battery that fills to `level`, green when charging, warning at 20% and below. */
  function BatteryMeter(p) {
    var lv = Math.max(0, Math.min(100, p.level || 0));
    var tone = p.charging ? "var(--success)" : lv <= 10 ? "var(--danger)" : lv <= 20 ? "var(--warning)" : "currentColor";
    return h("span", { className: "ec-batt", role: "meter", "aria-label": "Battery " + lv + "%" + (p.charging ? ", charging" : ""), "aria-valuenow": lv },
      h("span", { className: "ec-batt-body" }, h("i", { style: { width: lv + "%", background: tone } })),
      p.charging ? h(Icon, { name: "bolt", size: 12, className: "ec-batt-bolt" }) : null,
      p.label === false ? null : h("span", null, lv + "%"));
  }

  /** How the phone reaches the laptop: Wi-Fi for everything, Bluetooth for calls and clipboard. */
  function LinkPills(p) {
    var wifi = p.wifi !== false, bt = p.bluetooth !== false;
    return h("span", { className: "ec-links" },
      h("span", { className: cx("ec-link", !wifi && "ec-link-off") }, h(Icon, { name: "wifi", size: 14 }), wifi ? "Wi-Fi · " + (p.network || "home") : "No Wi-Fi"),
      h("span", { className: cx("ec-link", !bt && "ec-link-off") }, h(Icon, { name: "bluetooth", size: 14 }), bt ? "Bluetooth" : "Bluetooth off"));
  }

  // ------------------------------------------------------------ connect pieces
  /** The laptop at the top of Home: drawing, name, connection, its battery. */
  function LaptopCard(p) {
    var s = p.state || "connected", away = s === "away";
    return h("div", { className: cx("ec-lapcard", away && "ec-lapcard-away") },
      h("div", { className: "ec-lapcard-stage" }, h(LaptopDevice, { state: away ? "away" : p.locked ? "locked" : "connected", width: p.deviceWidth || 236 })),
      h("div", { className: "ec-lapcard-facts" },
        h("div", { className: "ec-lapcard-top" },
          away ? h(StatePill, null, "Not nearby · 2 h ago") : h(StatePill, { tone: "success" }, "Connected"),
          away ? null : h(BatteryMeter, { level: p.battery === undefined ? 64 : p.battery, charging: p.charging !== false })),
        h("h2", { className: "ec-lapcard-name" }, h("span", { className: "ec-world", "aria-hidden": "true" }), LAPTOP),
        away ? h("p", { className: "ec-lapcard-hint" }, "Open the laptop on the same Wi-Fi. Calls and clipboard also work over Bluetooth when it's in range.")
          : h(LinkPills, { wifi: p.wifi, bluetooth: p.bluetooth, network: p.network })));
  }

  var ACTIONS = [
    { icon: "upload", label: "Send files", sub: "Photos, PDFs, anything" },
    { icon: "clipboard", label: "Send clipboard", sub: "What you copied last" },
    { icon: "external", label: "Open on laptop", sub: "A link, in its browser" },
    { icon: "lock", label: "Lock laptop", sub: "Locks the screen now" }];
  /** Two-by-two quick actions on Home. */
  function ActionGrid(p) {
    var items = p.items || ACTIONS;
    return h("div", { className: "ec-actions", role: "group", "aria-label": "Quick actions" },
      items.map(function (a, i) {
        return h("button", { key: i, type: "button", className: "ec-action", disabled: p.disabled },
          h("span", { className: "ec-action-icon" }, h(Icon, { name: a.icon, size: 20 })),
          h("span", { className: "ec-action-label" }, a.label),
          h("span", { className: "ec-action-sub" }, a.sub));
      }));
  }

  /** One thing that crossed the shared clipboard. */
  function ClipItem(p) {
    var to = p.dir !== "from";
    var text = p.sensitive ? "•••• •••• ••••" : p.text;
    return h("div", { className: cx("ec-clip", p.sensitive && "ec-clip-secret") },
      h("span", { className: cx("ec-clip-dir", to ? "ec-clip-to" : "ec-clip-from"), title: to ? "Phone to laptop" : "Laptop to phone" }, h(Icon, { name: to ? "arrow-up" : "arrow-down", size: 16 })),
      p.image ? h("img", { className: "ec-clip-img", src: photoSrc(p.image, "screenshot"), alt: "" }) : null,
      h("span", { className: "ec-clip-text" },
        h("span", { className: "ec-clip-body" }, p.image ? "Screenshot · 1080 × 2340" : text),
        h("span", { className: "ec-clip-meta" }, (to ? "To laptop" : "From laptop") + " · " + (p.when || "now") + (p.via === "bt" ? " · Bluetooth" : ""),
          p.sensitive ? h(StatePill, { icon: "eye-off" }, "Hidden") : null)),
      h(IconButton, { icon: "copy", label: "Copy again", size: 18 }));
  }

  /** How the phone's clipboard reaches the laptop: Automatic, paused, or Tap to send. */
  function ClipModeCard(p) {
    var m = p.mode || "auto";
    var pill = m === "auto" ? h(StatePill, { tone: "success" }, "Automatic")
      : m === "paused" ? h(StatePill, { tone: "warning" }, "Paused")
      : h(StatePill, null, "Tap to send");
    var body = m === "auto" ? "Copy anything on the phone and it's on the laptop. Set up 2 Oct."
      : m === "paused" ? "The phone restarted. Allow log access once to switch automatic sending back on."
      : "Send with Send to laptop in the text menu, the quick-settings tile, or the button below.";
    return h("div", { className: cx("ec-mode", "ec-mode-" + m) },
      h("div", { className: "ec-mode-head" }, h(Icon, { name: "clipboard", size: 20 }), h("strong", null, "Phone → laptop"), h("span", { className: "ec-grow" }), pill),
      h("p", { className: "ec-mode-body" }, body),
      m === "paused" ? h(Button, { variant: "primary", icon: "refresh", block: true }, "Resume automatic")
        : m === "manual" ? h(Button, { icon: "bolt", block: true }, "Make it automatic") : null,
      h("p", { className: "ec-mode-foot" }, h(Icon, { name: "check", size: 14 }), "Laptop → phone is always automatic."));
  }

  /** A file going to or coming from the laptop. */
  function TransferRow(p) {
    var going = p.progress !== undefined && p.progress < 100;
    var sub = p.failed ? h("span", { className: "ec-danger-ink" }, "Stopped · Wi-Fi dropped")
      : going ? (p.rate || "2.5 of 4.1 MB · 18 MB/s")
      : (p.size || "212 KB") + " · " + (p.dir === "from" ? "from laptop" : "to laptop") + " · " + (p.when || "2 min ago");
    return h(ListRow, {
      lead: h(FileIcon, { name: iconFor(p.name), size: 36 }), title: p.name, sub: sub,
      below: going ? h(ProgressBar, { value: p.progress }) : null,
      trailing: going ? h(IconButton, { icon: "close", label: "Stop", size: 18 }) : p.failed ? h(Button, { size: "sm" }, "Retry") : h(IconButton, { icon: "external", label: "Open", size: 18 })
    });
  }

  /** One Android permission: what it unlocks, its state, and how to grant it. */
  function PermissionRow(p) {
    var trail = p.granted ? h(StatePill, { tone: "success" }, "Allowed") : h(Button, { size: "sm", variant: p.optional ? undefined : "primary" }, p.action || "Allow");
    return h(ListRow, { icon: p.icon, tint: p.granted ? "ok" : null, title: p.title, sub: p.why, trailing: trail });
  }

  /** Steps down the screen: done ones collapse to a line, the current one opens. */
  function StepList(p) {
    var cur = p.current || 0;
    return h("ol", { className: "ec-steps" }, (p.steps || []).map(function (s, i) {
      var state = i < cur ? "done" : i === cur ? "now" : "todo";
      return h("li", { key: i, className: "ec-step ec-step-" + state },
        h("span", { className: "ec-step-n" }, state === "done" ? h(Icon, { name: "check", size: 13, strokeWidth: 3 }) : i + 1),
        h("div", { className: "ec-step-main" },
          h("span", { className: "ec-step-title" }, s.title),
          state === "now" && s.body ? h("div", { className: "ec-step-body" }, s.body) : null,
          state === "done" && s.done ? h("span", { className: "ec-step-done" }, s.done) : null));
    }));
  }

  /** Progress through onboarding: one bar per step, like the EchoFiles stepper. */
  function Stepper(p) {
    var cur = p.current || 0, n = p.count || 5;
    var bars = [];
    for (var i = 0; i < n; i++) bars.push(h("i", { key: i, className: i < cur ? "ec-bar-done" : i === cur ? "ec-bar-now" : undefined }));
    return h("div", { className: "ec-stepper", role: "progressbar", "aria-valuenow": cur + 1, "aria-valuemax": n, "aria-label": "Step " + (cur + 1) + " of " + n },
      h("span", { className: "ec-stepper-bars" }, bars),
      h("span", { className: "ec-stepper-text" }, "Step " + (cur + 1) + " of " + n + (p.label ? " · " + p.label : "")));
  }

  function qrCells(seed) {
    var n = 25, out = [], x = seed || 7;
    function finder(r, c) { return (r < 7 && c < 7) || (r < 7 && c >= n - 7) || (r >= n - 7 && c < 7); }
    for (var r = 0; r < n; r++) for (var c = 0; c < n; c++) {
      if (finder(r, c)) continue;
      x = (x * 1103515245 + 12345) & 0x7fffffff;
      if ((x >> 16) & 1) out.push([r, c]);
    }
    return out;
  }
  var QR = qrCells(11);
  /** The camera pointed at the code EchoFiles shows: the code, accent corner brackets, one status line. */
  function QrViewfinder(p) {
    var u = 6, o = 20;
    var finders = [[0, 0], [0, 18], [18, 0]].map(function (f, i) {
      return h("g", { key: i },
        h("rect", { x: o + f[1] * u, y: o + f[0] * u, width: 7 * u, height: 7 * u, fill: "#111" }),
        h("rect", { x: o + f[1] * u + u, y: o + f[0] * u + u, width: 5 * u, height: 5 * u, fill: "#fff" }),
        h("rect", { x: o + f[1] * u + 2 * u, y: o + f[0] * u + 2 * u, width: 3 * u, height: 3 * u, fill: "#111" }));
    });
    return h("div", { className: cx("ec-viewfinder", p.found && "ec-viewfinder-found") },
      h("div", { className: "ec-viewfinder-cam" },
        h("svg", { viewBox: "0 0 190 190", className: "ec-viewfinder-code", "aria-hidden": "true" },
          h("rect", { width: 190, height: 190, fill: "#fff" }),
          QR.map(function (c, i) { return h("rect", { key: i, x: o + c[1] * u, y: o + c[0] * u, width: u, height: u, fill: "#111" }); }),
          finders),
        h("span", { className: "ec-corner ec-corner-tl" }), h("span", { className: "ec-corner ec-corner-tr" }),
        h("span", { className: "ec-corner ec-corner-bl" }), h("span", { className: "ec-corner ec-corner-br" })),
      h("p", { className: "ec-viewfinder-text" }, p.found ? h("span", null, h(Icon, { name: "check", size: 16 }), "Found " + LAPTOP) : "Point at the code in EchoFiles → Connect phone"));
  }

  /** The four-pair code both screens show while pairing. */
  function PairCode(p) {
    var code = p.code || ["4F", "9A", "2C", "71"];
    return h("div", { className: "ec-paircode", "aria-label": "Pairing code " + code.join(" ") }, code.map(function (c, i) { return h("span", { key: i }, c); }));
  }

  /** A call running through the laptop's mic and speakers. */
  function CallBar(p) {
    return h("div", { className: "ec-callbar", role: "status" },
      h("span", { className: "ec-callbar-icon" }, h(Icon, { name: "call", size: 18 })),
      h("span", { className: "ec-callbar-text" },
        h("strong", null, (p.who || "Priya") + " · " + (p.time || "03:12")),
        h("span", null, "On laptop mic and speakers · Bluetooth")),
      h(Button, { size: "sm" }, "Use phone"));
  }

  // ------------------------------------------------------------ Android system surfaces (system font, not ours)
  function SystemNotification(p) {
    return h("div", { className: "ec-sysnote", style: { fontFamily: SYS_SANS } },
      h("div", { className: "ec-sysnote-head" },
        h("span", { className: "ec-sysnote-app" }, h(Icon, { name: "laptop", size: 11, strokeWidth: 2.4 })),
        h("span", null, "EchoConnect · now"), h("span", { className: "ec-grow" }), h(Icon, { name: "chevron-down", size: 14 })),
      h("strong", { className: "ec-sysnote-title" }, p.title || "Connected to " + LAPTOP),
      h("span", { className: "ec-sysnote-text" }, p.text || "Wi-Fi · Bluetooth · clipboard automatic"),
      h("div", { className: "ec-sysnote-actions" }, (p.actions || ["Send clipboard", "Disconnect"]).map(function (a, i) { return h("button", { key: i, type: "button" }, a); })));
  }

  function QuickTile(p) {
    return h("button", { type: "button", className: cx("ec-qstile", p.on && "ec-qstile-on"), style: { fontFamily: SYS_SANS } },
      h("span", { className: "ec-qstile-icon" }, h(Icon, { name: p.icon || "clipboard", size: 18 })),
      h("span", { className: "ec-qstile-text" }, h("strong", null, p.label || "Send clipboard"), h("span", null, p.sub || "EchoConnect")));
  }

  function SelectionMenu(p) {
    return h("div", { className: "ec-selmenu", role: "menu", style: { fontFamily: SYS_SANS } },
      ["Cut", "Copy", "Paste", "Send to laptop"].map(function (a, i) {
        return h("button", { key: i, type: "button", role: "menuitem", className: a === "Send to laptop" ? "ec-selmenu-ours" : undefined }, a === "Send to laptop" ? h(Icon, { name: "laptop", size: 14 }) : null, a);
      }),
      h("button", { type: "button", role: "menuitem", "aria-label": "More" }, h(Icon, { name: "more-vertical", size: 16 })));
  }

  function SystemDialog(p) {
    return h("div", { className: "ec-sysdialog", role: "alertdialog", style: { fontFamily: SYS_SANS } },
      h(Icon, { name: "terminal", size: 22 }),
      h("strong", null, p.title || "Allow EchoConnect to access all device logs?"),
      h("p", null, p.body || "Apps use logs to find and fix issues. Some logs may contain sensitive information, so only allow apps you trust."),
      h("div", { className: "ec-sysdialog-actions" }, (p.actions || ["Allow one-time access", "Don't allow"]).map(function (a, i) { return h("button", { key: i, type: "button" }, a); })));
  }

  // ------------------------------------------------------------ stand-in pictures (shared painter with EchoFiles)
  var SCENES = [
    ["#ff9a5a", "#c2477d", "#2b1846", "#ffd27a"], ["#7cc6ff", "#3b6fd8", "#13285c", "#fff6c9"], ["#ffcf8a", "#ff7b54", "#3a1f3d", "#fff1c1"],
    ["#a7e3c4", "#3d9a7a", "#0f3b36", "#f4ffd8"], ["#c9b6ff", "#6a4cd6", "#1d1647", "#ffe0f2"], ["#ffd6e0", "#e0637f", "#41203a", "#fff5e0"]];
  function photoSrc(i, kind) {
    var s = SCENES[i % SCENES.length], svg;
    if (kind === "screenshot") {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 90 160"><rect width="90" height="160" fill="#101320"/><rect x="6" y="10" width="40" height="5" rx="2" fill="#e8ecff"/><rect x="6" y="22" width="78" height="34" rx="5" fill="' + s[1] + '"/>' +
        '<rect x="6" y="62" width="78" height="10" rx="3" fill="#232842"/><rect x="6" y="76" width="60" height="10" rx="3" fill="#232842"/><rect x="6" y="140" width="78" height="12" rx="6" fill="' + s[0] + '"/></svg>';
    } else {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice"><rect width="100" height="100" fill="' + s[0] + '"/><rect y="62" width="100" height="38" fill="' + s[1] + '"/>' +
        '<circle cx="' + (30 + (i * 17) % 45) + '" cy="38" r="11" fill="' + s[3] + '"/><path d="M0 70 22 50l14 12 20-22 22 24 22-14v50H0z" fill="' + s[2] + '"/></svg>';
    }
    return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
  }

  // ------------------------------------------------------------ screens
  function Scroll(p) { return h("div", { className: "ec-scroll" }, p.children); }
  function App(p) { return h("div", { className: "ec-app" }, p.children); }

  var CLIPS = [
    { dir: "to", text: "https://maps.app.goo.gl/x7KdQ2", when: "1 min ago" },
    { dir: "from", text: "ssh aditya@build-box -p 2222", when: "6 min ago" },
    { dir: "to", sensitive: true, when: "22 min ago" },
    { dir: "to", image: 1, when: "40 min ago" },
    { dir: "from", text: "cargo run -p echofiles-phone --example fakephone", when: "1 h ago", via: "bt" }];

  /** Home: the laptop, what to do with it, the clipboard and the latest transfers. */
  function HomeScreen(p) {
    var s = p.state || "connected", away = s === "away";
    return h(App, null,
      h(AppBar, { title: "EchoConnect", actions: [{ icon: "qr", label: "Pair another laptop" }] }),
      h(Scroll, null,
        h(LaptopCard, { state: away ? "away" : "connected", locked: p.locked }),
        s === "call" ? h(CallBar, null) : null,
        h(ActionGrid, { disabled: away }),
        h(ListGroup, { title: "Clipboard", action: "See all" },
          h(ClipItem, CLIPS[0]), h(ClipItem, CLIPS[1])),
        h(ListGroup, { title: "Transfers", action: "See all" },
          h(TransferRow, { name: "IMG_20261002_1031.jpg", progress: 62 }),
          h(TransferRow, { name: "boarding-pass.pdf", dir: "from", when: "12 min ago" }))),
      h(BottomNav, { active: 0, badges: { 2: 1 } }));
  }

  /** Clipboard: how phone → laptop works, Send now, and the history both ways. */
  function ClipboardScreen(p) {
    return h(App, null,
      h(AppBar, { title: "Clipboard", actions: [{ icon: "trash", label: "Clear history" }] }),
      h(Scroll, null,
        h(ClipModeCard, { mode: p.mode || "auto" }),
        h(Button, { variant: p.mode === "manual" ? "primary" : undefined, icon: "send", block: true, size: "lg" }, "Send clipboard now"),
        h(SegmentedControl, { label: "Show", block: true, options: [{ value: "all", text: "All" }, { value: "to", text: "To laptop" }, { value: "from", text: "From laptop" }] }),
        h(ListGroup, { title: "Today", foot: "History stays on this phone for 24 hours. Passwords from password managers are hidden and never kept." },
          CLIPS.map(function (c, i) { return h(ClipItem, Object.assign({ key: i }, c)); }))),
      h(BottomNav, { active: 1 }));
  }

  /** Transfers: what's moving now, then what moved, newest first. */
  function TransfersScreen() {
    return h(App, null,
      h(AppBar, { title: "Transfers", actions: [{ icon: "upload", label: "Send files" }] }),
      h(Scroll, null,
        h(ListGroup, { title: "Now" },
          h(TransferRow, { name: "VID_20261002_0912.mp4", progress: 38, rate: "31 of 84 MB · 22 MB/s · 3 s left" }),
          h(TransferRow, { name: "IMG_20261002_1031.jpg", progress: 62 })),
        h(ListGroup, { title: "Today" },
          h(TransferRow, { name: "boarding-pass.pdf", dir: "from", when: "12 min ago" }),
          h(TransferRow, { name: "Q3-final.xlsx", size: "1.4 MB", dir: "from", when: "09:40" }),
          h(TransferRow, { name: "voice-note.opus", size: "88 KB", failed: true })),
        h(ListGroup, { title: "Yesterday", foot: "Files from the laptop save to Download/EchoConnect. Files you send land in ~/Downloads/Phone on the laptop." },
          h(TransferRow, { name: "scan-lease.pdf", size: "2.1 MB", when: "Yesterday 18:22" }),
          h(TransferRow, { name: "echoconnect-debug.apk", size: "14 MB", dir: "from", when: "Yesterday 11:05" }))),
      h(BottomNav, { active: 2 }));
  }

  /** Settings: the laptop, what's shared, receiving, look, and about. */
  function SettingsScreen() {
    return h(App, null,
      h(AppBar, { title: "Settings" }),
      h(Scroll, null,
        h(ListGroup, { title: "Laptop" },
          h(ListRow, { lead: h(LaptopDevice, { width: 52 }), title: LAPTOP, sub: "Paired 2 Oct · EchoFiles 0.4" }),
          h(ListRow, { icon: "wifi", title: "Wi-Fi", sub: "Files, photos, texts, notifications", trailing: h(StatePill, { tone: "success" }, "home") }),
          h(ListRow, { icon: "bluetooth", title: "Bluetooth", sub: "Calls and clipboard, even off Wi-Fi", trailing: h(StatePill, { tone: "success" }, "On") }),
          h(ListRow, { icon: "unlink", title: "Forget this laptop", sub: "Pair again with its code to undo", chevron: true })),
        h(ListGroup, { title: "Shared with the laptop" },
          h(ListRow, { icon: "bell", title: "Notifications", sub: "Shown on the laptop; reply from there", trailing: h(Switch, { defaultChecked: true, label: "Notifications" }) }),
          h(ListRow, { icon: "message", title: "Messages", sub: "Read and send texts from the laptop", trailing: h(Switch, { defaultChecked: true, label: "Messages" }) }),
          h(ListRow, { icon: "call", title: "Calls on the laptop", sub: "Answer on laptop connects Bluetooth audio", trailing: h(Switch, { defaultChecked: true, label: "Calls on the laptop" }) }),
          h(ListRow, { icon: "image", title: "Files and photos", sub: "Browse the phone from EchoFiles", trailing: h(Switch, { defaultChecked: true, label: "Files and photos" }) }),
          h(ListRow, { icon: "clipboard", title: "Clipboard", sub: "Automatic · set up 2 Oct", chevron: true })),
        h(ListGroup, { title: "Receiving" },
          h(ListRow, { icon: "download", title: "Save files to", sub: "Download/EchoConnect", chevron: true }),
          h(ListRow, { icon: "check", title: "Accept files automatically", sub: "Off asks before each file", trailing: h(Switch, { defaultChecked: true, label: "Accept files automatically" }) })),
        h(ListGroup, { title: "Look" },
          h(ListRow, { icon: "sliders", title: "Theme", sub: "Match laptop uses its Omarchy theme", below: h(SegmentedControl, { label: "Theme", block: true, value: "laptop", options: [{ value: "laptop", text: "Match laptop" }, { value: "system", text: "Phone" }] }) })),
        h(ListGroup, { title: "This phone" },
          h(ListRow, { icon: "shield", title: "Permissions", sub: "8 of 9 allowed", chevron: true }),
          h(ListRow, { icon: "battery", title: "Stay connected", sub: "Never sleeping on Samsung", trailing: h(StatePill, { tone: "success" }, "Set") })),
        h(ListGroup, { title: "About", foot: "EchoConnect and EchoFiles are open source (GPL-3.0). Read every line at github.com/adityavardhansharma/EchoFiles_Linux." },
          h(ListRow, { icon: "info", title: "EchoConnect 0.1.0", sub: "Android 14 · One UI 6.1" }),
          h(ListRow, { icon: "code", title: "Source code", sub: "github.com/adityavardhansharma/EchoFiles_Linux", trailing: h(Icon, { name: "external", size: 18 }) }))),
      h(BottomNav, { active: 3 }));
  }

  var PERMS = [
    { icon: "bell", title: "Notification access", why: "Shows your notifications on the laptop", granted: true },
    { icon: "message", title: "SMS", why: "Read and send texts from the laptop", granted: true },
    { icon: "users", title: "Contacts", why: "Names instead of numbers in texts and calls", granted: true },
    { icon: "call", title: "Phone", why: "Caller name on the laptop; answer from there", granted: true },
    { icon: "bluetooth", title: "Nearby devices", why: "Bluetooth for calls and clipboard", granted: true },
    { icon: "image", title: "All files access", why: "Browse the phone from EchoFiles", granted: true },
    { icon: "clipboard", title: "Display over other apps", why: "Lets automatic clipboard read what you copy", granted: false, action: "Open setting" },
    { icon: "battery", title: "Battery: Unrestricted", why: "Stays connected with the screen off", granted: true },
    { icon: "bell", title: "Notifications", why: "The small always-on Connected notification", granted: true }];
  /** Permissions: each one, what it unlocks, and its state; nothing hidden. */
  function PermissionsScreen() {
    return h(App, null,
      h(AppBar, { title: "Permissions", back: true }),
      h(Scroll, null,
        h("p", { className: "ec-lede" }, "Each permission unlocks one feature. Turn a feature off in Settings and EchoConnect stops using its permission."),
        h(ListGroup, { title: "Allowed · 8 of 9" }, PERMS.map(function (pm, i) { return h(PermissionRow, Object.assign({ key: i }, pm)); })),
        h(Banner, { title: "On Samsung", icon: "battery" }, " Settings → Battery → Background usage limits → Never sleeping apps → add EchoConnect. Otherwise One UI may disconnect it overnight.")));
  }

  var SETUP_STEPS = [
    { title: "Turn on Developer options", done: "On",
      body: [h("p", { key: "a" }, "Settings → About phone → Software information → tap ", h("strong", null, "Build number"), " 7 times. Enter your PIN when asked."),
        h(Button, { key: "b", icon: "external", block: true }, "Open Software information")] },
    { title: "Turn on Wireless debugging", done: "On",
      body: [h("p", { key: "a" }, "Settings → Developer options → ", h("strong", null, "Wireless debugging"), " → Allow on this Wi-Fi. Then tap ", h("strong", null, "Pair device with QR code"), "."),
        h(Button, { key: "b", icon: "external", block: true }, "Open Developer options")] },
    { title: "Scan the code on the laptop", done: "Permission granted",
      body: [h("p", { key: "a" }, "On the laptop: EchoFiles → Settings → Phone → ", h("strong", null, "Make clipboard automatic"), ". Scan its code from the Wireless debugging screen."),
        h("div", { key: "b", className: "ec-wait" }, h(Spinner, null), "Waiting for " + LAPTOP + "…")] },
    { title: "Allow Display over other apps", done: "Allowed",
      body: [h("p", { key: "a" }, "A normal setting. It lets EchoConnect open an invisible window for a moment to read what you copied."),
        h(Button, { key: "b", variant: "primary", icon: "external", block: true }, "Open the setting")] },
    { title: "Turn Developer options off", done: "Off · banking apps work normally",
      body: [h("p", { key: "a" }, "The permission stays. Banking and payment apps only check whether Developer options are on now."),
        h(Button, { key: "b", variant: "primary", icon: "external", block: true }, "Open Developer options"),
        h("div", { key: "c", className: "ec-wait" }, h(StatePill, { tone: "warning" }, "Still on"), "EchoConnect checks again on its own.")] }];
  /** The one-time Automatic clipboard setup, run together with EchoFiles on the laptop. */
  function ClipboardSetupScreen(p) {
    var st = useState(p.step === undefined ? 2 : p.step);
    return h(App, null,
      h(AppBar, { title: "Automatic clipboard", back: true }),
      h(Scroll, null,
        h("p", { className: "ec-lede" }, "About 2 minutes, once. Afterwards you copy on the phone and paste on the laptop — no taps."),
        h(StepList, { steps: SETUP_STEPS, current: st[0] }),
        h("div", { className: "ec-setup-nav" },
          h(Button, { variant: "ghost", onClick: function () { st[1](Math.max(0, st[0] - 1)); } }, "Back"),
          h("span", { className: "ec-grow" }),
          h(Button, { onClick: function () { st[1](Math.min(5, st[0] + 1)); } }, st[0] >= 4 ? "Finish" : "Next step")),
        h(Banner, { title: "What EchoConnect reads", icon: "shield" },
          " Only the one line Android writes when a clipboard read is blocked. It never stores or sends anything else from the log. EchoConnect is open source — check the code at github.com/adityavardhansharma/EchoFiles_Linux."),
        h(Banner, { title: "After a restart", icon: "refresh" }, " Android asks once: Allow EchoConnect to access all device logs? Tap Allow one-time access and it works until the next restart."),
        h(Button, { variant: "ghost", block: true }, "Use Tap to send instead")));
  }

  var PAIR_STEPS = ["Welcome", "Scan", "Check code", "Permissions", "Clipboard"];
  /** First run: welcome, scan the laptop's code, check the code, permissions, clipboard choice. */
  function PairingScreen(p) {
    var st = useState(p.step || 0);
    var step = st[0], next = function () { st[1](Math.min(5, step + 1)); };
    var body, foot;
    if (step === 0) {
      body = [h("div", { key: "a", className: "ec-hero" }, h(LaptopDevice, { width: 260 })),
        h("h1", { key: "b", className: "ec-display" }, "Your phone, on your laptop"),
        h("p", { key: "c", className: "ec-lede" }, "Files, photos, clipboard, texts, notifications and calls between this phone and EchoFiles — over your Wi-Fi, with Bluetooth for calls."),
        h("ul", { key: "d", className: "ec-ticks" }, ["Nothing leaves your network", "Encrypted, paired once", "Open source"].map(function (t, i) { return h("li", { key: i }, h(Icon, { name: "check", size: 16 }), t); }))];
      foot = h(Button, { variant: "primary", size: "lg", block: true, icon: "scan", onClick: next }, "Scan the laptop's code");
    } else if (step === 1) {
      body = [h("p", { key: "a", className: "ec-lede" }, "On the laptop, open EchoFiles and click ", h("strong", null, "Connect phone"), " in the sidebar. A code appears."),
        h(QrViewfinder, { key: "b", found: p.found })];
      foot = h(Button, { variant: "ghost", block: true }, "Type the laptop's address instead");
    } else if (step === 2) {
      body = [h("p", { key: "a", className: "ec-lede" }, "Check the laptop shows the same code. Pairing is approved on both at once."),
        h(PairCode, { key: "b" }),
        h("div", { key: "c", className: "ec-wait" }, h(Spinner, null), "Pairing with " + LAPTOP + "…")];
      foot = [h(Button, { key: "a", variant: "primary", size: "lg", block: true, onClick: next }, "Codes match"), h(Button, { key: "b", variant: "ghost", block: true }, "They don't match")];
    } else if (step === 3) {
      body = [h("p", { key: "a", className: "ec-lede" }, "Allow what you want on the laptop. Skip any — you can turn it on later."),
        h(ListGroup, { key: "b" }, PERMS.slice(0, 6).map(function (pm, i) { return h(PermissionRow, Object.assign({ key: i }, pm, { granted: i < 3 })); }))];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: next }, "Continue");
    } else if (step === 4) {
      body = [h("p", { key: "a", className: "ec-lede" }, "How should what you copy on the phone reach the laptop?"),
        h("button", { key: "b", type: "button", className: "ec-choice", "aria-pressed": "true" },
          h("span", { className: "ec-choice-head" }, h(Icon, { name: "bolt", size: 18 }), h("strong", null, "Automatic"), h(StatePill, { tone: "accent" }, "Recommended")),
          h("span", null, "Copy on the phone, paste on the laptop. One-time setup with the laptop, about 2 minutes; Developer options go back off after.")),
        h("button", { key: "c", type: "button", className: "ec-choice", "aria-pressed": "false" },
          h("span", { className: "ec-choice-head" }, h(Icon, { name: "send", size: 18 }), h("strong", null, "Tap to send")),
          h("span", null, "Use Send to laptop in the text menu, the quick-settings tile, or Share. No setup.")),
        h("p", { key: "d", className: "ec-note" }, h(Icon, { name: "check", size: 14 }), "Laptop → phone is automatic either way.")];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: next }, "Set up Automatic");
    } else {
      body = [h("div", { key: "a", className: "ec-hero" }, h(LaptopDevice, { width: 260 })),
        h("h1", { key: "b", className: "ec-display" }, "Connected"),
        h("p", { key: "c", className: "ec-lede" }, LAPTOP + " can now see this phone. EchoConnect keeps a small notification while it's connected.")];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: function () { st[1](0); } }, "Done");
    }
    return h(App, null,
      h("div", { className: "ec-flowhead" }, step > 0 && step < 5 ? h(IconButton, { icon: "arrow-left", label: "Back", onClick: function () { st[1](step - 1); } }) : h("span", { className: "ec-flowhead-pad" }),
        step < 5 ? h(Stepper, { current: step, count: 5, label: PAIR_STEPS[step] }) : h(Stepper, { current: 5, count: 5, label: "Done" })),
      h(Scroll, null, body),
      h("div", { className: "ec-flowfoot" }, foot));
  }

  /** Full screen while the laptop rings the phone. */
  function RingScreen() {
    return h("div", { className: "ec-ring" },
      h("div", { className: "ec-ring-mark", "aria-hidden": "true" }, h("i"), h("i"), h("span", null, h(Icon, { name: "phone-ring", size: 40 }))),
      h("h1", { className: "ec-display" }, "Ringing"),
      h("p", { className: "ec-lede" }, LAPTOP + " is looking for this phone. Full volume, even on silent; the flashlight blinks too."),
      h("span", { className: "ec-grow" }),
      h(Button, { variant: "primary", size: "lg", block: true, icon: "close" }, "Stop ringing"),
      h("p", { className: "ec-ring-foot" }, "Any volume key stops it too."));
  }

  /** Share → EchoConnect from any app: what goes, where it lands, Send. */
  function ShareSheetScreen() {
    return h(App, null,
      h("div", { className: "ec-gallery", "aria-hidden": "true" }, [0, 1, 2, 3, 4, 5, 0, 2, 4, 1, 3, 5, 2, 0, 1].map(function (k, i) {
        return h("span", { key: i, className: [1, 4, 6].indexOf(i) >= 0 ? "ec-gallery-sel" : undefined }, h("img", { src: photoSrc(k), alt: "" }));
      })),
      h("div", { className: "ec-scrim" }),
      h(BottomSheet, { title: "Send to " + LAPTOP, icon: "laptop",
        footer: [h(Button, { key: "c", variant: "ghost" }, "Cancel"), h(Button, { key: "s", variant: "primary", icon: "send" }, "Send 3 photos")] },
        h("div", { className: "ec-sharethumbs" }, [1, 4, 0].map(function (k, i) { return h("img", { key: i, src: photoSrc(k), alt: "" }); })),
        h(ListRow, { icon: "folder", title: "Lands in ~/Downloads/Phone", sub: "11.8 MB · over Wi-Fi, about 1 second" }),
        h(ListRow, { icon: "check", title: "Open on the laptop when done", trailing: h(Switch, { label: "Open on the laptop when done" }) })));
  }

  /** The Android-side pieces EchoConnect adds: notification, tile, text menu, log prompt. */
  function SystemSurfaces() {
    return h("div", { className: "ec-surfaces" },
      h("figure", null, h(PhoneFrame, { height: 620 },
        h("div", { className: "ec-shade" },
          h("div", { className: "ec-shade-tiles" }, h(QuickTile, { on: true }), h(QuickTile, { icon: "wifi", label: "Wi-Fi", sub: "home", on: true }), h(QuickTile, { icon: "bluetooth", label: "Bluetooth", sub: LAPTOP, on: true }), h(QuickTile, { icon: "moon", label: "Do not disturb", sub: "Synced with laptop" })),
          h(SystemNotification, null),
          h(SystemNotification, { title: "Priya · on laptop", text: "Call on laptop mic and speakers · 03:12", actions: ["Use phone", "Hang up"] }))),
        h("figcaption", null, "Notification shade: the always-on notification, the Send clipboard tile, a call on the laptop.")),
      h("figure", null, h(PhoneFrame, { height: 620 },
        h(App, null,
          h("div", { className: "ec-chat" },
            h("span", { className: "ec-chat-bubble" }, "Wi-Fi password for the office is "),
            h("span", { className: "ec-chat-bubble" }, h("mark", null, "blue-otter-42"), " — don't share it"),
            h(SelectionMenu, null)))),
        h("figcaption", null, "Text selection: Send to laptop sits next to Copy — the no-setup way.")),
      h("figure", null, h(PhoneFrame, { height: 620, dim: true },
        h("div", { className: "ec-dialogstage" }, h(SystemDialog, null))),
        h("figcaption", null, "After a restart: Android's log prompt. Allow one-time access resumes Automatic.")));
  }

  /** Every screen side by side: the app at a glance. */
  function AppMap() {
    var cells = [["Home", h(HomeScreen, null)], ["Home · call on laptop", h(HomeScreen, { state: "call" })], ["Clipboard", h(ClipboardScreen, null)], ["Transfers", h(TransfersScreen, null)],
      ["Settings", h(SettingsScreen, null)], ["Pairing · scan", h(PairingScreen, { step: 1, found: true })], ["Automatic clipboard", h(ClipboardSetupScreen, null)], ["Ringing", h(RingScreen, null)]];
    return h("div", { className: "ec-map" }, cells.map(function (c, i) {
      return h("figure", { key: i }, h(PhoneFrame, { height: 720 }, c[1]), h("figcaption", null, c[0]));
    }));
  }

  window.EchoConnect = {
    Icon: Icon, FileIcon: FileIcon, iconFor: iconFor, Button: Button, IconButton: IconButton, Switch: Switch, Checkbox: Checkbox,
    SegmentedControl: SegmentedControl, TextField: TextField, StatePill: StatePill, Spinner: Spinner, ProgressBar: ProgressBar,
    Snackbar: Snackbar, Banner: Banner, AppBar: AppBar, BottomNav: BottomNav, SectionHeader: SectionHeader, ListGroup: ListGroup,
    ListRow: ListRow, BottomSheet: BottomSheet, Dialog: Dialog, PhoneFrame: PhoneFrame, LaptopDevice: LaptopDevice,
    BatteryMeter: BatteryMeter, LinkPills: LinkPills, LaptopCard: LaptopCard, ActionGrid: ActionGrid, ClipItem: ClipItem,
    ClipModeCard: ClipModeCard, TransferRow: TransferRow, PermissionRow: PermissionRow, StepList: StepList, Stepper: Stepper,
    QrViewfinder: QrViewfinder, PairCode: PairCode, CallBar: CallBar, SystemNotification: SystemNotification, QuickTile: QuickTile,
    SelectionMenu: SelectionMenu, SystemDialog: SystemDialog, HomeScreen: HomeScreen, ClipboardScreen: ClipboardScreen,
    TransfersScreen: TransfersScreen, SettingsScreen: SettingsScreen, PermissionsScreen: PermissionsScreen,
    ClipboardSetupScreen: ClipboardSetupScreen, PairingScreen: PairingScreen, RingScreen: RingScreen,
    ShareSheetScreen: ShareSheetScreen, SystemSurfaces: SystemSurfaces, AppMap: AppMap, photoSrc: photoSrc
  };
})();

})();
