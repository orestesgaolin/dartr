var a = 'unterminated
var b = "bad escape \x \u{110000} \u12";
var c = '''multi
line ${a}''';
var d = r'raw \n';
var e = 'nested ${'inner ${x}'}';
var f = '$';
var g = "${}";
