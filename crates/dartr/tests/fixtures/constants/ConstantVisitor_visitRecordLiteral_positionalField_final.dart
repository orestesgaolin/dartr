
final bar = '';
(String, ) foo() => const (bar, );
//                         ^^^
// [diag.nonConstantRecordField] The fields in a const record literal must be constants.
