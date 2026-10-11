sealed class Entity {}

abstract class ResultSet<Self extends ResultSet<Self>> extends Entity {}

class Table extends ResultSet<Table> {}

int check(Entity e) {
  return switch (e) {
    Table() => 1,
    ResultSet() => 2,
  };
}
