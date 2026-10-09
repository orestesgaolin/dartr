
const c = identical(ExList<ExInt>, List<int>);

extension type const ExInt(int value) implements int {}
extension type const ExList<T>(List<T> value) implements List<T> {}
