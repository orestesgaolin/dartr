// Classes that look like Flutter widgets, for closing labels.
import 'generated/excluded.dart';

export 'generated/excluded.dart';

class Widget {
  const Widget();
}

class Container extends Widget {
  final Widget? child;
  const Container({this.child});
  const Container.empty() : child = null;
}

class Row extends Widget {
  final List<Widget> children;
  const Row({this.children = const []});
}

class Text extends Widget {
  final String data;
  const Text(this.data);
}

Widget build() {
  return Container(
    child: Row(
      children: <Widget>[
        Text('a'),
        Container(
          child: Text('b'),
        ),
        Container.empty(),
      ],
    ),
  );
}

Widget buildNew() => new Container(
      child: const Row(
        children: [
          Text('c'),
        ],
      ),
    );
