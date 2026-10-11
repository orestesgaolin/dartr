void configure({void Function(int)? setup}) {}
void configurePositional(void Function(int) setup) {}

void run() {
  configure(setup: (x) => {x.toString()});
  configurePositional((x) => {x.toString()});
}
