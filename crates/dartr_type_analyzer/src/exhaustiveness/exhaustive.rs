// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/exhaustive.dart

use std::collections::VecDeque;

use indexmap::IndexSet;

use super::key::Key;
use super::path::Path;
use super::profile;
use super::space::{SingleSpace, Space};
use super::static_type::{ObjectPropertyLookup, StaticType, StaticTypeArena};
use super::witness::{Predicate, Witness};

/// Returns `true` if [case_spaces] exhaustively covers all possible values of
/// [value_space].
pub fn is_exhaustive(
    field_lookup: &dyn ObjectPropertyLookup,
    value_space: &Space,
    case_spaces: &[Space],
) -> bool {
    check_exhaustiveness(field_lookup, value_space, case_spaces).is_none()
}

/// Checks the [case_spaces] representing a series of switch cases to see if
/// they exhaustively cover all possible values of the matched [value_type].
/// Also checks to see if any case can't be matched because it's covered by
/// previous cases.
///
/// [case_is_guarded] should be a list of booleans indicating whether each case
/// in [case_spaces] has an associated `when` clause.
///
/// If any unreachable cases are found, information about them is appended to
/// [case_unreachabilities]. (If `None` is passed for [case_unreachabilities],
/// then no information about unreachable cases is generated).
///
/// If the switch cases are not fully exhaustive, details about how they fail
/// to be exhaustive are returned using a data structure of type
/// [NonExhaustiveness]; otherwise `None` is returned.
///
/// Note that if a non-null value is returned, that doesn't necessarily mean
/// that an error should be reported; the caller still must check whether the
/// switch has a `default` clause and whether the scrutinee type is an "always
/// exhaustive" type.
pub fn compute_exhaustiveness(
    field_lookup: &dyn ObjectPropertyLookup,
    value_type: StaticType,
    case_is_guarded: &[bool],
    case_spaces: &[Space],
    mut case_unreachabilities: Option<&mut Vec<CaseUnreachability>>,
) -> Option<NonExhaustiveness> {
    let checker = Checker::new(field_lookup);

    let value_pattern = Space::new(Path::root(), value_type);
    let mut case_rows: Vec<Vec<Space>> = vec![];

    for i in 0..case_spaces.len() {
        let case_row = vec![case_spaces[i].clone()];
        if let Some(case_unreachabilities) = case_unreachabilities.as_deref_mut()
            && i > 0
        {
            // See if this case is covered by previous ones.
            if checker
                .unmatched(&case_rows, &case_row, &[], false)
                .is_none()
            {
                case_unreachabilities.push(CaseUnreachability::new(
                    value_type,
                    case_spaces.to_vec(),
                    i,
                ));
            }
        }
        if !case_is_guarded[i] {
            case_rows.push(case_row);
        }
    }

    let witnesses = checker.unmatched(&case_rows, &[value_pattern], &[], true);
    witnesses.map(|witnesses| NonExhaustiveness::new(value_type, case_spaces.to_vec(), witnesses))
}

/// Determines if [cases] is exhaustive over all values contained by
/// [value_space]. If so, returns `None`. Otherwise, returns a list of
/// [Witness]s of values that aren't matched by anything in [cases].
pub fn check_exhaustiveness(
    field_lookup: &dyn ObjectPropertyLookup,
    value_space: &Space,
    cases: &[Space],
) -> Option<Vec<Witness>> {
    let checker = Checker::new(field_lookup);

    // TODO(johnniwinther): Perform reachability checking.
    let case_rows: Vec<Vec<Space>> = cases.iter().map(|space| vec![space.clone()]).collect();

    // Uncomment this to have it print out the witness for non-exhaustive
    // matches.
    // if (witnesses != null) witnesses.forEach(print);

    checker.unmatched(&case_rows, std::slice::from_ref(value_space), &[], true)
}

/// Dart `_Checker`.
struct Checker<'a> {
    property_lookup: &'a dyn ObjectPropertyLookup,
    types: &'a dyn StaticTypeArena,
}

impl<'a> Checker<'a> {
    fn new(property_lookup: &'a dyn ObjectPropertyLookup) -> Checker<'a> {
        Checker {
            property_lookup,
            types: property_lookup.static_types(),
        }
    }

    /// Tries to find a pattern containing at least one value matched by
    /// [value_patterns] that is not matched by any of the patterns in
    /// [case_rows].
    ///
    /// If found, returns it. This is a witness example showing that
    /// [case_rows] is not exhaustive over all values in [value_patterns]. If it
    /// returns `None`, then [case_rows] exhaustively covers [value_patterns].
    fn unmatched(
        &self,
        case_rows: &[Vec<Space>],
        value_patterns: &[Space],
        witness_predicates: &[Predicate],
        return_multiple_witnesses: bool,
    ) -> Option<Vec<Witness>> {
        debug_assert!(
            case_rows
                .iter()
                .all(|element| element.len() == value_patterns.len()),
            "Value patterns: {:?}, case rows: {:?}.",
            value_patterns,
            case_rows
        );
        profile::count("_unmatched", None);
        // If there are no more columns, then we've tested all the predicates we
        // have to test.
        if value_patterns.is_empty() {
            // If there are still any rows left, then it means every remaining
            // value will go to one of those rows' bodies, so we have
            // successfully matched.
            if !case_rows.is_empty() {
                return None;
            }

            // If we ran out of rows too, then it means [witness_predicates] is
            // now a complete description of at least one value that slipped
            // past all the rows.
            return Some(vec![Witness::new(witness_predicates.to_vec())]);
        } else if case_rows.is_empty() && !return_multiple_witnesses {
            // We have no more cases that can match the witness, so unless we
            // want to multiple witnesses, we return directly.
            //
            // This will return the shortest possible witness and will for
            // instance return `(E.b, _)` instead of `(E.b, E.a)` for
            //
            //     enum E { a, b }
            //     m(E e) => switch (e) { (E.a, _) => 0, };
            //
            // and `C(f1: int())` instead of `C(f1: int(), f2: int())` for
            //
            //     abstract class C { num f1, f2; }
            //     m(C c) => switch (e) { C(f1: double(), f2: double()) => 0, };
            //
            // Additionally, it avoids a degenerate case occurring only when
            // checking for unreachable cases. In this mode, the value space is
            // created from cases that are not included in the case rows. This
            // means that the value space visited with an empty set of case rows
            // left, can be exponential in the size of the case. For instance if
            // we have
            //
            //     sealed class S {}
            //     class S1 extends S {}
            //     class S2 extends S {
            //       String? f1, f2, f3, f4, f5, f6, f7, f8;
            //     }
            //     m(S s) => {
            //         S1() => 0,
            //         S2(:var f1, :var f2, :var f3, :var f4,
            //            :var f5, :var f6, :var f7, :var f8) => 1,
            //       };
            //
            // then in order to check that the second case is not unreachable
            // after the first case, we would otherwise check all combinations
            // of `S2` with fields values `String` or `null` for fields `f1` to
            // `f8`, instead of just stopping with the shortest witness `S2()`.
            //
            // If we want to compute multiple witness, which we only do for the
            // top-most value space, we continue the search for longer
            // witnesses. This means that if we have
            //
            //     enum E { a, b }
            //     m(E e) => switch (e) {};
            //
            // we do not simply return `_` as the witness, but instead the
            // witnesses `E.a` and `E.b`. We want this for the quick-fix that
            // adds all enum values or sealed class subclasses in case of an
            // empty switch.
            return Some(vec![Witness::new(witness_predicates.to_vec())]);
        }

        // Look down the first column of tests.
        let first_value_patterns = &value_patterns[0];

        let mut keys_of_interest: IndexSet<Key> = IndexSet::new();
        for case_row in case_rows {
            for single_space in case_row[0].single_spaces() {
                keys_of_interest.extend(single_space.additional_properties.keys().cloned());
            }
        }
        for first_value_pattern in first_value_patterns.single_spaces() {
            let context_type = first_value_pattern.type_;
            let mut stack: VecDeque<StaticType> = VecDeque::from([first_value_pattern.type_]);
            let mut witnesses: Option<Vec<Witness>> = None;
            while let Some(type_) = stack.pop_front() {
                if self.types.is_subtype_of(type_, StaticType::NEVER_TYPE) {
                    // Don't try to exhaust the Never type.
                    continue;
                }
                if self.types.is_sealed(type_) {
                    let result = self.filter_by_type(
                        context_type,
                        type_,
                        case_rows,
                        first_value_pattern,
                        value_patterns,
                        witness_predicates,
                        first_value_patterns.path(),
                        // We don't use the witnesses, so only compute one.
                        false,
                    );
                    if result.is_none() {
                        // This type was fully handled so no need to test its
                        // subtypes.
                    } else {
                        // The type was not fully handled so we must allow for
                        // handling of individual subtypes.
                        stack.extend(self.types.get_subtypes(type_, &keys_of_interest));
                    }
                } else {
                    let result = self.filter_by_type(
                        context_type,
                        type_,
                        case_rows,
                        first_value_pattern,
                        value_patterns,
                        witness_predicates,
                        first_value_patterns.path(),
                        // Don't collect multiple witnesses for to avoid
                        // combinatorial explosion. For instance returning
                        //
                        //    (E.a, E.b), (E.a, E.c) ... (E.z, E.z) // 675 witnesses
                        //
                        // for
                        //
                        //    enum E { a, b, ..., z }
                        //    method((E, E) r) => switch (r) { (E.a, E.a) => 0, };
                        //
                        false,
                    );

                    // If we found a witness for a subtype that no rows match,
                    // then we can stop. There may be others but we don't need
                    // to find more.
                    if let Some(result) = result {
                        witnesses.get_or_insert_with(Vec::new).extend(result);
                        if !return_multiple_witnesses {
                            return witnesses;
                        }
                    }
                }
            }
            if witnesses.is_some() {
                return witnesses;
            }
        }

        // If we get here, no subtype yielded a witness, so we must have matched
        // everything.
        None
    }

    fn filter_by_type(
        &self,
        context_type: StaticType,
        type_: StaticType,
        case_rows: &[Vec<Space>],
        first_single_space_value: &SingleSpace,
        value_spaces: &[Space],
        witness_predicates: &[Predicate],
        path: &Path,
        return_multiple_witnesses: bool,
    ) -> Option<Vec<Witness>> {
        profile::count("_filterByType", None);
        // Extend the witness with the type we're matching.
        let mut extended_witness: Vec<Predicate> = witness_predicates.to_vec();
        extended_witness.push(Predicate::new(path.clone(), context_type, type_));

        // 1) Discard any rows that might not match because the column's type
        // isn't a subtype of the value's type.  We only keep rows that *must*
        // match because a row that could potentially fail to match will not
        // help us prove exhaustiveness.
        //
        // 2) Expand any unions in the first column. This can (deliberately)
        // produce duplicate rows in remainingRows.
        let mut remaining_row_first_single_spaces: Vec<&SingleSpace> = vec![];
        let mut remaining_rows: Vec<&Vec<Space>> = vec![];
        for row in case_rows {
            let first_space = &row[0];

            for first_single_space in first_space.single_spaces() {
                // If the row's type is a supertype of the value pattern's type
                // then it must match.
                if self.types.is_subtype_of(type_, first_single_space.type_) {
                    remaining_row_first_single_spaces.push(first_single_space);
                    remaining_rows.push(row);
                }
            }
        }

        // We have now filtered by the type test of the first column of
        // patterns, but some of those may also have field subpatterns. If so,
        // lift those out so we can recurse into them.
        let mut property_keys: IndexSet<Key> = first_single_space_value
            .properties
            .keys()
            .cloned()
            .collect();
        for first_pattern in &remaining_row_first_single_spaces {
            property_keys.extend(first_pattern.properties.keys().cloned());
        }

        let mut additional_property_keys: IndexSet<Key> = first_single_space_value
            .additional_properties
            .keys()
            .cloned()
            .collect();
        for first_pattern in &remaining_row_first_single_spaces {
            additional_property_keys.extend(first_pattern.additional_properties.keys().cloned());
        }

        // Sorting isn't necessary, but makes the behavior deterministic.
        let mut sorted_property_keys: Vec<Key> = property_keys.into_iter().collect();
        sorted_property_keys.sort_by(|a, b| a.compare_to(b, self.types));
        let mut sorted_additional_property_keys: Vec<Key> =
            additional_property_keys.into_iter().collect();
        sorted_additional_property_keys.sort_by(|a, b| a.compare_to(b, self.types));

        // Remove the first column from the value list and replace it with any
        // expanded fields.
        let mut new_value_spaces = self.expand_properties(
            &sorted_property_keys,
            &sorted_additional_property_keys,
            first_single_space_value,
            type_,
            path,
        );
        new_value_spaces.extend(value_spaces.iter().skip(1).cloned());

        // Remove the first column from each row and replace it with any
        // expanded fields.
        let mut new_remaining_rows: Vec<Vec<Space>> = Vec::with_capacity(remaining_rows.len());
        for i in 0..remaining_rows.len() {
            let mut row = self.expand_properties(
                &sorted_property_keys,
                &sorted_additional_property_keys,
                remaining_row_first_single_spaces[i],
                remaining_row_first_single_spaces[i].type_,
                path,
            );
            row.extend(remaining_rows[i].iter().skip(1).cloned());
            new_remaining_rows.push(row);
        }

        // Proceed to the next column.
        self.unmatched(
            &new_remaining_rows,
            &new_value_spaces,
            &extended_witness,
            return_multiple_witnesses,
        )
    }

    /// Given a list of [property_keys] and [additional_property_keys], and a
    /// [single_space], generates a list of single spaces, one for each named
    /// property and additional property key.
    ///
    /// When [single_space] contains a property with that name or an additional
    /// property with the key, extracts it into the resulting list. Otherwise,
    /// the [single_space] doesn't care about that property, so inserts a
    /// default [Space] that matches all values for the property. If the [type_]
    /// doesn't know about the property, the static type of the property is read
    /// from [Key].
    ///
    /// In other words, this unpacks a set of properties so that the main
    /// algorithm can add them to the worklist.
    fn expand_properties(
        &self,
        property_keys: &[Key],
        additional_property_keys: &[Key],
        single_space: &SingleSpace,
        type_: StaticType,
        path: &Path,
    ) -> Vec<Space> {
        profile::count("_expandProperties", None);
        let mut result: Vec<Space> = vec![];
        for key in property_keys {
            if let Some(property) = single_space.properties.get(key) {
                result.push(property.clone());
            } else {
                // This pattern doesn't test this property, so add a pattern for
                // the property that matches all values. This way the columns
                // stay aligned.
                let mut property_type =
                    self.types
                        .get_property_type(type_, self.property_lookup, key);
                if property_type.is_none()
                    && let Key::Extension(extension_key) = key
                {
                    property_type = Some(extension_key.type_);
                }
                // TODO(johnniwinther): Enable this assert when extension
                // members are handled.
                /*assert(propertyType != null,
                "Type $type does not have a type for property $key");*/
                result.push(Space::new(
                    path.add(key.clone()),
                    property_type.unwrap_or(StaticType::NULLABLE_OBJECT),
                ));
            }
        }
        for key in additional_property_keys {
            if let Some(property) = single_space.additional_properties.get(key) {
                result.push(property.clone());
            } else {
                // This pattern doesn't test this property, so add a pattern for
                // the property that matches all values. This way the columns
                // stay aligned.
                // TODO(johnniwinther): Enable this assert when extension
                // members are handled.
                // assert(type.getAdditionalPropertyType(key) != null,
                //    "Type $type does not have a type for additional property $key");
                result.push(Space::new(
                    path.add(key.clone()),
                    self.types
                        .get_additional_property_type(type_, key)
                        .unwrap_or(StaticType::NULLABLE_OBJECT),
                ));
            }
        }
        result
    }
}

/// Recursively expands [type_] with its subtypes if it's sealed.
///
/// Otherwise, just returns [type_].
pub fn expand_sealed_subtypes(
    types: &dyn StaticTypeArena,
    type_: StaticType,
    keys_of_interest: &IndexSet<Key>,
) -> Vec<StaticType> {
    profile::count("expandSealedSubtypes", None);
    if !types.is_sealed(type_) {
        vec![type_]
    } else {
        let mut result: IndexSet<StaticType> = IndexSet::new();
        for subtype in types.get_subtypes(type_, keys_of_interest) {
            result.extend(expand_sealed_subtypes(types, subtype, keys_of_interest));
        }
        result.into_iter().collect()
    }
}

pub fn checking_order(
    types: &dyn StaticTypeArena,
    type_: StaticType,
    keys_of_interest: &IndexSet<Key>,
) -> Vec<StaticType> {
    let mut result: Vec<StaticType> = vec![];
    let mut pending: VecDeque<StaticType> = VecDeque::from([type_]);
    while let Some(type_) = pending.pop_front() {
        result.push(type_);
        if types.is_sealed(type_) {
            pending.extend(types.get_subtypes(type_, keys_of_interest));
        }
    }
    result
}

#[derive(Debug)]
pub struct NonExhaustiveness {
    pub value_type: StaticType,

    pub cases: Vec<Space>,

    pub witnesses: Vec<Witness>,
}

impl NonExhaustiveness {
    pub fn new(value_type: StaticType, cases: Vec<Space>, witnesses: Vec<Witness>) -> Self {
        NonExhaustiveness {
            value_type,
            cases,
            witnesses,
        }
    }

    /// Dart `NonExhaustiveness.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        format!(
            "{} is not exhaustively matched by {}.",
            types.name(self.value_type),
            self.cases
                .iter()
                .map(|c| c.to_text(types))
                .collect::<Vec<_>>()
                .join("|")
        )
    }
}

#[derive(Debug)]
pub struct CaseUnreachability {
    pub value_type: StaticType,
    pub cases: Vec<Space>,
    pub index: usize,
}

impl CaseUnreachability {
    pub fn new(value_type: StaticType, cases: Vec<Space>, index: usize) -> Self {
        CaseUnreachability {
            value_type,
            cases,
            index,
        }
    }

    /// Dart `CaseUnreachability.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        format!(
            "Case #{} {} is unreachable.",
            self.index + 1,
            self.cases[self.index].to_text(types)
        )
    }
}
