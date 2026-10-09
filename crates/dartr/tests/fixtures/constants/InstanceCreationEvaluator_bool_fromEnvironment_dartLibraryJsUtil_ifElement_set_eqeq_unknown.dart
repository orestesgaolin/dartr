
const a = bool.fromEnvironment('dart.library.js_util');
const b = {3, if (a) ...[1] else ...[1, 2], 4};
const left = {3, if (a) ...[1] else ...[1, 2], 4} == b;
const right = b == {3, if (a) ...[1] else ...[1, 2], 4};
