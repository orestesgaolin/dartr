void main() {
  var a = 1, b = 2, _c = 3;
  print("a=$a b=${b} sum=${a + b}");
  print('nested ${"inner ${a + "${b}"}"} done');
  print("$this.x $_c $a$b ${a}${b}");
  print("dollar at end $");
  print("dollar digit $1 and space $ x");
  print('''multi $a
line ${b + 1}
''');
  print("""triple "quotes" ${'"'} $a""");
  print("map ${{'k': 1}['k']} list ${[1, 2][0]}");
  print("escaped \$a \${b} \\$a");
  print('${() { return "}"; }()}');
  print("$a$");
  print('${a}$b${c}d');
}
