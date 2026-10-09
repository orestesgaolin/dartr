
const x = {'string': 1};
const Map<String, int> alwaysInclude = {
  'anotherString': 0,
  ...x,
};
