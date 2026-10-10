// A stub of the old `package:test_api` location of `fail`, which the
// use_test_throws_matchers rule checks (current test_api versions export
// `fail` from package:matcher).
Never fail(String message) => throw Exception(message);
