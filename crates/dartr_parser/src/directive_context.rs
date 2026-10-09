// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/directive_context.dart

use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::TokenId;

use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Dart `DirectiveState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DirectiveState {
    Unknown,
    Script,
    Library,
    ImportAndExport,
    Part,
    PartOf,
    Declarations,
}

/// Dart `DirectiveContext`. Dart passes the parser to the `check*`
/// methods; so does this port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectiveContext {
    pub is_enhanced_parts_feature_enabled: bool,
    pub state: DirectiveState,
}

impl DirectiveContext {
    pub fn new(is_enhanced_parts_feature_enabled: bool) -> Self {
        DirectiveContext {
            is_enhanced_parts_feature_enabled,
            state: DirectiveState::Unknown,
        }
    }

    pub fn check_script_tag<L: Listener>(&mut self, _parser: &mut Parser<L>, _token: TokenId) {
        if self.state == DirectiveState::Unknown {
            self.state = DirectiveState::Script;
            return;
        }
        panic!("Internal error: Unexpected script tag.");
    }

    pub fn check_declaration(&mut self) {
        if self.state != DirectiveState::PartOf {
            self.state = DirectiveState::Declarations;
        }
    }

    pub fn check_export<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) {
        match self.state {
            DirectiveState::Unknown
            | DirectiveState::Script
            | DirectiveState::Library
            | DirectiveState::ImportAndExport => self.state = DirectiveState::ImportAndExport,
            DirectiveState::Part => {
                parser.report_recoverable_error(token, diag::export_after_part())
            }
            DirectiveState::PartOf => {
                if self.is_enhanced_parts_feature_enabled {
                    self.state = DirectiveState::ImportAndExport;
                } else {
                    parser.report_recoverable_error(token, diag::non_part_of_directive_in_part());
                }
            }
            DirectiveState::Declarations => {
                parser.report_recoverable_error(token, diag::directive_after_declaration())
            }
        }
    }

    pub fn check_import<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) {
        match self.state {
            DirectiveState::Unknown
            | DirectiveState::Script
            | DirectiveState::Library
            | DirectiveState::ImportAndExport => self.state = DirectiveState::ImportAndExport,
            DirectiveState::Part => {
                parser.report_recoverable_error(token, diag::import_after_part())
            }
            DirectiveState::PartOf => {
                if self.is_enhanced_parts_feature_enabled {
                    self.state = DirectiveState::ImportAndExport;
                } else {
                    parser.report_recoverable_error(token, diag::non_part_of_directive_in_part());
                }
            }
            DirectiveState::Declarations => {
                parser.report_recoverable_error(token, diag::directive_after_declaration())
            }
        }
    }

    pub fn check_library<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) {
        if self.state < DirectiveState::Library {
            self.state = DirectiveState::Library;
            return;
        }
        if self.state == DirectiveState::Library {
            parser.report_recoverable_error(token, diag::multiple_library_directives());
        } else if self.state == DirectiveState::PartOf {
            parser.report_recoverable_error(token, diag::non_part_of_directive_in_part());
        } else {
            parser.report_recoverable_error(token, diag::library_directive_not_first());
        }
    }

    pub fn check_part<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) {
        match self.state {
            DirectiveState::Unknown
            | DirectiveState::Script
            | DirectiveState::Library
            | DirectiveState::ImportAndExport
            | DirectiveState::Part => self.state = DirectiveState::Part,
            DirectiveState::PartOf => {
                if self.is_enhanced_parts_feature_enabled {
                    self.state = DirectiveState::ImportAndExport;
                } else {
                    parser.report_recoverable_error(token, diag::non_part_of_directive_in_part());
                }
            }
            DirectiveState::Declarations => {
                parser.report_recoverable_error(token, diag::directive_after_declaration())
            }
        }
    }

    pub fn check_part_of<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) {
        if self.state == DirectiveState::Unknown {
            self.state = DirectiveState::PartOf;
            return;
        }
        if self.state == DirectiveState::PartOf {
            parser.report_recoverable_error(token, diag::part_of_twice());
        } else {
            parser.report_recoverable_error(token, diag::non_part_of_directive_in_part());
        }
    }
}
