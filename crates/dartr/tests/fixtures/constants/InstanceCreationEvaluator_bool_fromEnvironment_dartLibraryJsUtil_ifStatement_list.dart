
const a = bool.fromEnvironment('dart.library.js_util');
const x = [3, if (a) ...[1] else ...[1, 2], 4];
