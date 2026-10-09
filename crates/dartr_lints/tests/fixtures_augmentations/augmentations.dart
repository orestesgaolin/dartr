part of 'library.dart';
augment abstract int BAD_NAME;
augment abstract var untyped;
augment void function({String? BAD_PARAMETER});
augment void positional(String? BAD_POSITIONAL);
augment class Example {
  augment abstract int BAD_FIELD;
  augment abstract var untypedField;
  augment void method({String? BAD_PARAMETER});
}
