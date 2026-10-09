
final bar = '';
({String bar, }) foo() => const (bar: bar, );
//                                    ^^^
// [diag.nonConstantRecordField] The fields in a const record literal must be constants.
