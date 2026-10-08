// Dart source: pkg/analyzer/lib/src/generated/element_walker.dart

//! `ElementWalker`: yields the linked fragments of a container (a library
//! fragment, a class, an executable, ...) in declaration order, so that the
//! element binding visitor can pair them with the declaration nodes.
//!
//! Difference: the Dart `get*` methods throw an `IndexError` when the
//! fragments and the declarations do not match (a bug of the linker). Here
//! they return `None`, and the caller skips the declaration.

use dartr_element::{
    ClassFragment, ConstructorFragment, Ctx, EnumFragment, ExecutableFragment, ExtensionFragment,
    ExtensionTypeFragment, FId, FormalParameterFragment, FragmentFlags, FragmentId, GetterFragment,
    LibraryFragment, MixinFragment, SetterFragment, TypeAliasFragment, TypeParameterFragment,
    VariableFragment,
};

/// A list of child fragments and the index of the next one (Dart `_xs` and
/// `_xIndex`).
#[derive(Debug)]
struct Cursor<T> {
    items: Option<Vec<T>>,
    index: usize,
}

impl<T: Copy> Cursor<T> {
    fn none() -> Self {
        Cursor {
            items: None,
            index: 0,
        }
    }

    fn of(items: Vec<T>) -> Self {
        Cursor {
            items: Some(items),
            index: 0,
        }
    }

    fn next(&mut self) -> Option<T> {
        let item = self.items.as_ref()?.get(self.index).copied();
        if item.is_some() {
            self.index += 1;
        }
        item
    }
}

/// Dart `ElementWalker`.
#[derive(Debug)]
pub struct ElementWalker {
    /// Dart `fragment`: the fragment whose children are walked.
    pub fragment: FragmentId,
    classes: Cursor<FId<ClassFragment>>,
    constructors: Cursor<FId<ConstructorFragment>>,
    enums: Cursor<FId<EnumFragment>>,
    extensions: Cursor<FId<ExtensionFragment>>,
    extension_types: Cursor<FId<ExtensionTypeFragment>>,
    functions: Cursor<FId<ExecutableFragment>>,
    getters: Cursor<FId<GetterFragment>>,
    mixins: Cursor<FId<MixinFragment>>,
    parameters: Cursor<FId<FormalParameterFragment>>,
    setters: Cursor<FId<SetterFragment>>,
    typedefs: Cursor<FId<TypeAliasFragment>>,
    type_parameters: Cursor<FId<TypeParameterFragment>>,
    variables: Cursor<FId<VariableFragment>>,
}

fn has(ctx: &Ctx<'_>, f: FragmentId, flag: FragmentFlags) -> bool {
    ctx.fragment_data(f).is_some_and(|d| d.flags.has(flag))
}

/// `fragments.where((f) => f.isOriginDeclaration)` for getters and setters.
fn origin_accessors<T: ?Sized>(ctx: &Ctx<'_>, items: &[FId<T>]) -> Vec<FId<T>> {
    items
        .iter()
        .copied()
        .filter(|f| {
            has(
                ctx,
                f.raw(),
                FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION,
            )
        })
        .collect()
}

/// `fields.where((f) => f.isOriginDeclaration)` (fields and top-level
/// variables), as variable fragments.
fn origin_variables<T: ?Sized>(ctx: &Ctx<'_>, items: &[FId<T>]) -> Vec<FId<VariableFragment>> {
    items
        .iter()
        .copied()
        .filter(|f| {
            has(
                ctx,
                f.raw(),
                FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
            )
        })
        .map(|f| FId::from_raw(f.raw()))
        .collect()
}

fn origin_constructors(
    ctx: &Ctx<'_>,
    items: &[FId<ConstructorFragment>],
) -> Vec<FId<ConstructorFragment>> {
    items
        .iter()
        .copied()
        .filter(|f| {
            has(
                ctx,
                f.raw(),
                FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION,
            )
        })
        .collect()
}

fn as_executables<T: ?Sized>(items: &[FId<T>]) -> Vec<FId<ExecutableFragment>> {
    items.iter().map(|f| FId::from_raw(f.raw())).collect()
}

impl ElementWalker {
    fn empty(fragment: FragmentId) -> ElementWalker {
        ElementWalker {
            fragment,
            classes: Cursor::none(),
            constructors: Cursor::none(),
            enums: Cursor::none(),
            extensions: Cursor::none(),
            extension_types: Cursor::none(),
            functions: Cursor::none(),
            getters: Cursor::none(),
            mixins: Cursor::none(),
            parameters: Cursor::none(),
            setters: Cursor::none(),
            typedefs: Cursor::none(),
            type_parameters: Cursor::none(),
            variables: Cursor::none(),
        }
    }

    /// Dart `ElementWalker.forClass`.
    pub fn for_class(ctx: &Ctx<'_>, fragment: FId<ClassFragment>) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.constructors = if f
            .flags
            .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
        {
            Cursor::none()
        } else {
            Cursor::of(origin_constructors(ctx, &f.constructors))
        };
        w.functions = Cursor::of(as_executables(&f.methods));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.type_parameters = Cursor::of(f.type_params.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.fields));
        w
    }

    /// Dart `ElementWalker.forCompilationUnit`.
    pub fn for_compilation_unit(ctx: &Ctx<'_>, fragment: FId<LibraryFragment>) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.classes = Cursor::of(f.classes.clone());
        w.enums = Cursor::of(f.enums.clone());
        w.extensions = Cursor::of(f.extensions.clone());
        w.extension_types = Cursor::of(f.extension_types.clone());
        w.functions = Cursor::of(as_executables(&f.functions));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.mixins = Cursor::of(f.mixins.clone());
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.typedefs = Cursor::of(f.type_aliases.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.variables));
        w
    }

    /// Dart `ElementWalker.forEnum`.
    pub fn for_enum(ctx: &Ctx<'_>, fragment: FId<EnumFragment>) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.constructors = Cursor::of(origin_constructors(ctx, &f.constructors));
        w.functions = Cursor::of(as_executables(&f.methods));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.type_parameters = Cursor::of(f.type_params.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.fields));
        w
    }

    /// Dart `ElementWalker.forExecutable`.
    pub fn for_executable(ctx: &Ctx<'_>, fragment: FId<ExecutableFragment>) -> ElementWalker {
        let f = ctx.store(fragment.store()).executable_fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.functions = Cursor::of(Vec::new());
        w.parameters = Cursor::of(f.formal_params.clone());
        w.type_parameters = Cursor::of(f.type_params.clone());
        w
    }

    /// Dart `ElementWalker.forExtension`.
    pub fn for_extension(ctx: &Ctx<'_>, fragment: FId<ExtensionFragment>) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.functions = Cursor::of(as_executables(&f.methods));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.type_parameters = Cursor::of(f.type_params.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.fields));
        w
    }

    /// Dart `ElementWalker.forExtensionType`.
    pub fn for_extension_type(
        ctx: &Ctx<'_>,
        fragment: FId<ExtensionTypeFragment>,
    ) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.constructors = Cursor::of(f.constructors.clone());
        w.functions = Cursor::of(as_executables(&f.methods));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.type_parameters = Cursor::of(f.type_params.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.fields));
        w
    }

    /// Dart `ElementWalker.forGenericTypeAlias`.
    pub fn for_generic_type_alias(
        ctx: &Ctx<'_>,
        fragment: FId<TypeAliasFragment>,
    ) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.type_parameters = Cursor::of(f.type_params.clone());
        w
    }

    /// Dart `ElementWalker.forMixin`.
    pub fn for_mixin(ctx: &Ctx<'_>, fragment: FId<MixinFragment>) -> ElementWalker {
        let f = ctx.fragment(fragment);
        let mut w = ElementWalker::empty(fragment.raw());
        w.constructors = Cursor::of(origin_constructors(ctx, &f.constructors));
        w.functions = Cursor::of(as_executables(&f.methods));
        w.getters = Cursor::of(origin_accessors(ctx, &f.getters));
        w.setters = Cursor::of(origin_accessors(ctx, &f.setters));
        w.type_parameters = Cursor::of(f.type_params.clone());
        w.variables = Cursor::of(origin_variables(ctx, &f.fields));
        w
    }

    /// Dart `ElementWalker.forTypedef`.
    pub fn for_typedef(ctx: &Ctx<'_>, fragment: FId<TypeAliasFragment>) -> ElementWalker {
        ElementWalker::for_generic_type_alias(ctx, fragment)
    }

    /// Dart `getClass`.
    pub fn get_class(&mut self) -> Option<FId<ClassFragment>> {
        self.classes.next()
    }

    /// Dart `getConstructor`.
    pub fn get_constructor(&mut self) -> Option<FId<ConstructorFragment>> {
        self.constructors.next()
    }

    /// Dart `getEnum`.
    pub fn get_enum(&mut self) -> Option<FId<EnumFragment>> {
        self.enums.next()
    }

    /// Dart `getExtension`.
    pub fn get_extension(&mut self) -> Option<FId<ExtensionFragment>> {
        self.extensions.next()
    }

    /// Dart `getExtensionType`.
    pub fn get_extension_type(&mut self) -> Option<FId<ExtensionTypeFragment>> {
        self.extension_types.next()
    }

    /// Dart `getFunction`: the next top-level function, method, or local
    /// function.
    pub fn get_function(&mut self) -> Option<FId<ExecutableFragment>> {
        self.functions.next()
    }

    /// Dart `getGetter`.
    pub fn get_getter(&mut self) -> Option<FId<GetterFragment>> {
        self.getters.next()
    }

    /// Dart `getMixin`.
    pub fn get_mixin(&mut self) -> Option<FId<MixinFragment>> {
        self.mixins.next()
    }

    /// Dart `getParameter`.
    pub fn get_parameter(&mut self) -> Option<FId<FormalParameterFragment>> {
        self.parameters.next()
    }

    /// Dart `getSetter`.
    pub fn get_setter(&mut self) -> Option<FId<SetterFragment>> {
        self.setters.next()
    }

    /// Dart `getTypedef`.
    pub fn get_typedef(&mut self) -> Option<FId<TypeAliasFragment>> {
        self.typedefs.next()
    }

    /// Dart `getTypeParameter`.
    pub fn get_type_parameter(&mut self) -> Option<FId<TypeParameterFragment>> {
        self.type_parameters.next()
    }

    /// Dart `getVariable`: the next top-level variable, field, or local
    /// variable.
    pub fn get_variable(&mut self) -> Option<FId<VariableFragment>> {
        self.variables.next()
    }
}
