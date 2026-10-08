// Default types of type parameters (DefaultTypesBuilder).

class A1<X extends A1<X, Y>, Y extends A2<X, Y>> {}

class A2<X extends A1<X, Y>, Y extends A2<X, Y>> {}

void f<X extends A1<X, Y>, Y extends A2<X, Y>>() {}

void g<X extends Y, Y extends List<X>>() {}

class C1<T extends void Function<TT extends T>()> {}

class C2<T extends TT Function<TT extends T>()> {}

class C3<X, Y extends void Function<Z extends X>(Z)> {}

typedef F<T extends void Function<S extends T>(S)> = T Function();
