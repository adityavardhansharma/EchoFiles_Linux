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
