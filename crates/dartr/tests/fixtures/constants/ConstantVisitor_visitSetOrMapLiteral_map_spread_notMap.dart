
const x = ['string'];
const Map<String, int> alwaysInclude = {
  'anotherString': 0,
  ...x,
//   ^
// [diag.constSpreadExpectedMap] A map is expected in this spread.
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.
};
