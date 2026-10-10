// Batch B Flutter rules without hits in the corpora. The package config of
// bench/corpus/visible-app resolves package:flutter and package:test_api.
import 'package:flutter/material.dart';
import 'package:test_api/test_api.dart';

class NoKey extends StatelessWidget {
  NoKey();

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Container(width: 10, height: 10),
        Container(width: 10, height: 10, child: const Text('a')),
        SizedBox(width: double.infinity, height: double.infinity),
        SizedBox(width: 0, height: 0),
        Container(color: const Color(0xFF00), child: const Text('b')),
        Container(
          decoration: const BoxDecoration(color: Colors.red),
          child: const Text('c'),
        ),
        Padding(child: const Text('d'), padding: EdgeInsets.zero),
        const Text('e'),
      ],
    );
  }
}

class WithKey extends StatelessWidget {
  const WithKey({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

class _PrivateWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) => const SizedBox();
}

Widget privateWidget() => _PrivateWidget();

class MutableWidget extends StatelessWidget {
  MutableWidget({super.key, required this.value});
  final int value;

  @override
  Widget build(BuildContext context) => Text('$value');
}

Future<void> asyncUse(BuildContext context) async {
  await Future<void>.delayed(Duration.zero);
  Navigator.of(context).pop();
}

Future<void> guardedUse(BuildContext context) async {
  await Future<void>.delayed(Duration.zero);
  if (!context.mounted) return;
  Navigator.of(context).pop();
}

class StatefulUser extends StatefulWidget {
  const StatefulUser({super.key});

  @override
  State<StatefulUser> createState() => _StatefulUserState();
}

class _StatefulUserState extends State<StatefulUser> {
  Future<void> wrongMounted(BuildContext other) async {
    await Future<void>.delayed(Duration.zero);
    if (!mounted) return;
    Navigator.of(other).pop();
  }

  Future<void> stateContext() async {
    await Future<void>.delayed(Duration.zero);
    Navigator.of(context).pop();
  }

  void callback() {
    Future<void>.delayed(Duration.zero, () {
      Navigator.of(context).pop();
    });
  }

  @override
  Widget build(BuildContext context) => const SizedBox();
}

void throwsMatchers() {
  try {
    throw StateError('x');
    fail('expected');
  } catch (e) {
    print(e);
  }
}
