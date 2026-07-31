use evrel_ir::{
    BindingId, BindingKind, LocationId, ModuleBuilder, ModuleDependency, ModuleExport,
    ModuleExportName, ModuleImport, ModuleIr, ModuleKey, ModuleRequest, ModuleRequestKind,
    ModuleTarget, ProgramBindingId, ProgramIr,
};

use super::{ImportedBindingTarget, ProgramLinkage};

#[test]
fn resolves_transitive_indirect_and_star_exports() {
    let (source, source_binding) = module_with_local_export("value");

    let mut relay = ModuleIr::new();
    ModuleBuilder::new(&mut relay).add_export(ModuleExport::indirect(
        LocationId::UNKNOWN,
        "./source.js",
        [],
        name("value"),
        name("renamed"),
    ));

    let mut barrel = ModuleIr::new();
    ModuleBuilder::new(&mut barrel).add_export(ModuleExport::star(
        LocationId::UNKNOWN,
        "./relay.js",
        [],
    ));

    let mut consumer = ModuleIr::new();
    let imported = add_named_import(&mut consumer, "./barrel.js", "renamed");

    let mut program = ProgramIr::new();
    let source_id = program.add_module(ModuleKey::new("source"), source);
    let relay_id = program.add_module(ModuleKey::new("relay"), relay);
    let barrel_id = program.add_module(ModuleKey::new("barrel"), barrel);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);

    add_internal_dependency(
        &mut program,
        relay_id,
        ModuleRequestKind::ReExport,
        "./source.js",
        source_id,
    );
    add_internal_dependency(
        &mut program,
        barrel_id,
        ModuleRequestKind::ReExport,
        "./relay.js",
        relay_id,
    );
    add_internal_dependency(
        &mut program,
        consumer_id,
        ModuleRequestKind::StaticImport,
        "./barrel.js",
        barrel_id,
    );

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, imported)),
        Some(&ImportedBindingTarget::Binding(ProgramBindingId::new(
            source_id,
            source_binding,
        )))
    );
}

#[test]
fn resolves_exports_through_a_star_cycle() {
    let mut first = ModuleIr::new();
    ModuleBuilder::new(&mut first).add_export(ModuleExport::star(
        LocationId::UNKNOWN,
        "./second.js",
        [],
    ));

    let (mut second, second_binding) = module_with_local_export("value");
    ModuleBuilder::new(&mut second).add_export(ModuleExport::star(
        LocationId::UNKNOWN,
        "./first.js",
        [],
    ));

    let mut consumer = ModuleIr::new();
    let imported = add_named_import(&mut consumer, "./first.js", "value");

    let mut program = ProgramIr::new();
    let first_id = program.add_module(ModuleKey::new("first"), first);
    let second_id = program.add_module(ModuleKey::new("second"), second);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);

    add_internal_dependency(
        &mut program,
        first_id,
        ModuleRequestKind::ReExport,
        "./second.js",
        second_id,
    );
    add_internal_dependency(
        &mut program,
        second_id,
        ModuleRequestKind::ReExport,
        "./first.js",
        first_id,
    );
    add_internal_dependency(
        &mut program,
        consumer_id,
        ModuleRequestKind::StaticImport,
        "./first.js",
        first_id,
    );

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, imported)),
        Some(&ImportedBindingTarget::Binding(ProgramBindingId::new(
            second_id,
            second_binding,
        )))
    );
}

#[test]
fn resolves_namespace_reexports() {
    let source = ModuleIr::new();

    let mut relay = ModuleIr::new();
    ModuleBuilder::new(&mut relay).add_export(ModuleExport::namespace(
        LocationId::UNKNOWN,
        "./source.js",
        [],
        name("namespace"),
    ));

    let mut consumer = ModuleIr::new();
    let imported = add_named_import(&mut consumer, "./relay.js", "namespace");

    let mut program = ProgramIr::new();
    let source_id = program.add_module(ModuleKey::new("source"), source);
    let relay_id = program.add_module(ModuleKey::new("relay"), relay);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);

    add_internal_dependency(
        &mut program,
        relay_id,
        ModuleRequestKind::ReExport,
        "./source.js",
        source_id,
    );
    add_internal_dependency(
        &mut program,
        consumer_id,
        ModuleRequestKind::StaticImport,
        "./relay.js",
        relay_id,
    );

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, imported)),
        Some(&ImportedBindingTarget::Namespace(ModuleTarget::Internal(
            source_id,
        )))
    );
}

#[test]
fn rejects_ambiguous_star_exports() {
    let (left, _) = module_with_local_export("value");
    let (right, _) = module_with_local_export("value");

    let mut barrel = ModuleIr::new();
    let mut builder = ModuleBuilder::new(&mut barrel);
    builder.add_export(ModuleExport::star(LocationId::UNKNOWN, "./left.js", []));
    builder.add_export(ModuleExport::star(LocationId::UNKNOWN, "./right.js", []));

    let mut consumer = ModuleIr::new();
    let imported = add_named_import(&mut consumer, "./barrel.js", "value");

    let mut program = ProgramIr::new();
    let left_id = program.add_module(ModuleKey::new("left"), left);
    let right_id = program.add_module(ModuleKey::new("right"), right);
    let barrel_id = program.add_module(ModuleKey::new("barrel"), barrel);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);

    add_internal_dependency(
        &mut program,
        barrel_id,
        ModuleRequestKind::ReExport,
        "./left.js",
        left_id,
    );
    add_internal_dependency(
        &mut program,
        barrel_id,
        ModuleRequestKind::ReExport,
        "./right.js",
        right_id,
    );
    add_internal_dependency(
        &mut program,
        consumer_id,
        ModuleRequestKind::StaticImport,
        "./barrel.js",
        barrel_id,
    );

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, imported)),
        Some(&ImportedBindingTarget::Unresolved)
    );
}

#[test]
fn does_not_resolve_default_through_star_exports() {
    let (source, _) = module_with_local_export("default");

    let mut barrel = ModuleIr::new();
    ModuleBuilder::new(&mut barrel).add_export(ModuleExport::star(
        LocationId::UNKNOWN,
        "./source.js",
        [],
    ));

    let mut consumer = ModuleIr::new();
    let imported = add_default_import(&mut consumer, "./barrel.js");

    let mut program = ProgramIr::new();
    let source_id = program.add_module(ModuleKey::new("source"), source);
    let barrel_id = program.add_module(ModuleKey::new("barrel"), barrel);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);

    add_internal_dependency(
        &mut program,
        barrel_id,
        ModuleRequestKind::ReExport,
        "./source.js",
        source_id,
    );
    add_internal_dependency(
        &mut program,
        consumer_id,
        ModuleRequestKind::StaticImport,
        "./barrel.js",
        barrel_id,
    );

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, imported)),
        Some(&ImportedBindingTarget::Unresolved)
    );
}

#[test]
fn preserves_uninspectable_exports_and_missing_linkage() {
    let mut consumer = ModuleIr::new();
    let opaque = add_named_import(&mut consumer, "opaque", "value");
    let external = add_default_import(&mut consumer, "external");
    let missing = add_named_import(&mut consumer, "missing", "value");

    let mut program = ProgramIr::new();
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);
    program.add_dependency(ModuleDependency::new(
        consumer_id,
        ModuleRequest::new(ModuleRequestKind::StaticImport, "opaque", []),
        ModuleTarget::Opaque(ModuleKey::new("opaque")),
    ));
    program.add_dependency(ModuleDependency::new(
        consumer_id,
        ModuleRequest::new(ModuleRequestKind::StaticImport, "external", []),
        ModuleTarget::External(ModuleKey::new("external")),
    ));

    let linkage = ProgramLinkage::analyze(&program);

    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, opaque)),
        Some(&ImportedBindingTarget::OpaqueExport {
            module: ModuleKey::new("opaque"),
            name: "value".into(),
        })
    );
    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, external)),
        Some(&ImportedBindingTarget::ExternalExport {
            module: ModuleKey::new("external"),
            name: "default".into(),
        })
    );
    assert_eq!(
        linkage.imported_binding(ProgramBindingId::new(consumer_id, missing)),
        Some(&ImportedBindingTarget::Unresolved)
    );
}

fn module_with_local_export(exported: &str) -> (ModuleIr, BindingId) {
    let mut module = ModuleIr::new();
    let entry = module.entry_function();
    let mut builder = ModuleBuilder::new(&mut module);
    let binding = builder.create_binding(entry, exported, BindingKind::Const);
    builder.add_export(ModuleExport::local(
        LocationId::UNKNOWN,
        name(exported),
        binding,
    ));

    (module, binding)
}

fn add_named_import(module: &mut ModuleIr, source: &str, imported: &str) -> BindingId {
    let entry = module.entry_function();
    let mut builder = ModuleBuilder::new(module);
    let binding = builder.create_binding(entry, imported, BindingKind::Import);
    builder.add_import(ModuleImport::named(
        LocationId::UNKNOWN,
        source,
        [],
        name(imported),
        binding,
    ));

    binding
}

fn add_default_import(module: &mut ModuleIr, source: &str) -> BindingId {
    let entry = module.entry_function();
    let mut builder = ModuleBuilder::new(module);
    let binding = builder.create_binding(entry, "default", BindingKind::Import);
    builder.add_import(ModuleImport::default(
        LocationId::UNKNOWN,
        source,
        [],
        binding,
    ));

    binding
}

fn add_internal_dependency(
    program: &mut ProgramIr,
    importer: evrel_ir::ModuleId,
    kind: ModuleRequestKind,
    specifier: &str,
    target: evrel_ir::ModuleId,
) {
    program.add_dependency(ModuleDependency::new(
        importer,
        ModuleRequest::new(kind, specifier, []),
        ModuleTarget::Internal(target),
    ));
}

fn name(value: &str) -> ModuleExportName {
    ModuleExportName::Identifier(value.into())
}
