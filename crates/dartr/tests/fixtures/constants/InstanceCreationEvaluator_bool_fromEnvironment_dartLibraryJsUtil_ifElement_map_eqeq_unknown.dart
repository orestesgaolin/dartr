
const a = bool.fromEnvironment('dart.library.js_util');
const b = {3:'3', if (a) 1:'1' else 2:'2', 4:'4'};
const left = {3:'3', if (a) 1:'1' else 2:'2', 4:'4'} == b;
const right = b == {3:'3', if (a) 1:'1' else 2:'2', 4:'4'};
