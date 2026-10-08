// Dart source: sdk/lib/internal/sort.dart

//! The sort of the Dart VM (`List.sort`): insertion sort for up to 33
//! elements, dual-pivot quicksort above. It is not stable, so the order of
//! equal elements (for example a synthetic token and the node at the same
//! offset in `childEntities`) depends on the algorithm.

/// Dart `Sort.sort`.
pub fn dart_sort<E: Copy>(a: &mut [E], mut compare: impl FnMut(&E, &E) -> i64) {
    if a.is_empty() {
        return;
    }
    do_sort(a, 0, a.len() as isize - 1, &mut compare);
}

const INSERTION_SORT_THRESHOLD: isize = 32;

fn do_sort<E: Copy>(
    a: &mut [E],
    left: isize,
    right: isize,
    compare: &mut impl FnMut(&E, &E) -> i64,
) {
    if right - left <= INSERTION_SORT_THRESHOLD {
        insertion_sort(a, left, right, compare);
    } else {
        dual_pivot_quicksort(a, left, right, compare);
    }
}

fn insertion_sort<E: Copy>(
    a: &mut [E],
    left: isize,
    right: isize,
    compare: &mut impl FnMut(&E, &E) -> i64,
) {
    let mut i = left + 1;
    while i <= right {
        let el = a[i as usize];
        let mut j = i;
        while j > left && compare(&a[(j - 1) as usize], &el) > 0 {
            a[j as usize] = a[(j - 1) as usize];
            j -= 1;
        }
        a[j as usize] = el;
        i += 1;
    }
}

fn dual_pivot_quicksort<E: Copy>(
    a: &mut [E],
    left: isize,
    right: isize,
    compare: &mut impl FnMut(&E, &E) -> i64,
) {
    macro_rules! at {
        ($i:expr) => {
            a[($i) as usize]
        };
    }
    let sixth = (right - left + 1) / 6;
    let index1 = left + sixth;
    let index5 = right - sixth;
    let index3 = (left + right) / 2;
    let index2 = index3 - sixth;
    let index4 = index3 + sixth;

    let mut el1 = at!(index1);
    let mut el2 = at!(index2);
    let mut el3 = at!(index3);
    let mut el4 = at!(index4);
    let mut el5 = at!(index5);

    // Sort the five elements with a sorting network.
    if compare(&el1, &el2) > 0 {
        std::mem::swap(&mut el1, &mut el2);
    }
    if compare(&el4, &el5) > 0 {
        std::mem::swap(&mut el4, &mut el5);
    }
    if compare(&el1, &el3) > 0 {
        std::mem::swap(&mut el1, &mut el3);
    }
    if compare(&el2, &el3) > 0 {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el1, &el4) > 0 {
        std::mem::swap(&mut el1, &mut el4);
    }
    if compare(&el3, &el4) > 0 {
        std::mem::swap(&mut el3, &mut el4);
    }
    if compare(&el2, &el5) > 0 {
        std::mem::swap(&mut el2, &mut el5);
    }
    if compare(&el2, &el3) > 0 {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el4, &el5) > 0 {
        std::mem::swap(&mut el4, &mut el5);
    }

    let pivot1 = el2;
    let pivot2 = el4;

    at!(index1) = el1;
    at!(index3) = el3;
    at!(index5) = el5;

    at!(index2) = at!(left);
    at!(index4) = at!(right);

    let mut less = left + 1;
    let mut great = right - 1;

    let pivots_are_equal = compare(&pivot1, &pivot2) == 0;
    if pivots_are_equal {
        let pivot = pivot1;
        let mut k = less;
        while k <= great {
            let ak = at!(k);
            let mut comp = compare(&ak, &pivot);
            if comp == 0 {
                k += 1;
                continue;
            }
            if comp < 0 {
                if k != less {
                    at!(k) = at!(less);
                    at!(less) = ak;
                }
                less += 1;
            } else {
                loop {
                    comp = compare(&at!(great), &pivot);
                    if comp > 0 {
                        great -= 1;
                        continue;
                    } else if comp < 0 {
                        at!(k) = at!(less);
                        at!(less) = at!(great);
                        less += 1;
                        at!(great) = ak;
                        great -= 1;
                        break;
                    } else {
                        at!(k) = at!(great);
                        at!(great) = ak;
                        great -= 1;
                        break;
                    }
                }
            }
            k += 1;
        }
    } else {
        let mut k = less;
        while k <= great {
            let ak = at!(k);
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 < 0 {
                if k != less {
                    at!(k) = at!(less);
                    at!(less) = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 > 0 {
                    loop {
                        let comp = compare(&at!(great), &pivot2);
                        if comp > 0 {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            let comp = compare(&at!(great), &pivot1);
                            if comp < 0 {
                                at!(k) = at!(less);
                                at!(less) = at!(great);
                                less += 1;
                                at!(great) = ak;
                                great -= 1;
                            } else {
                                at!(k) = at!(great);
                                at!(great) = ak;
                                great -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
    }

    at!(left) = at!(less - 1);
    at!(less - 1) = pivot1;
    at!(right) = at!(great + 1);
    at!(great + 1) = pivot2;

    do_sort(a, left, less - 2, compare);
    do_sort(a, great + 2, right, compare);

    if pivots_are_equal {
        return;
    }

    if less < index1 && great > index5 {
        while compare(&at!(less), &pivot1) == 0 {
            less += 1;
        }
        while compare(&at!(great), &pivot2) == 0 {
            great -= 1;
        }
        let mut k = less;
        while k <= great {
            let ak = at!(k);
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 == 0 {
                if k != less {
                    at!(k) = at!(less);
                    at!(less) = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 == 0 {
                    loop {
                        let comp = compare(&at!(great), &pivot2);
                        if comp == 0 {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            let comp = compare(&at!(great), &pivot1);
                            if comp < 0 {
                                at!(k) = at!(less);
                                at!(less) = at!(great);
                                less += 1;
                                at!(great) = ak;
                                great -= 1;
                            } else {
                                at!(k) = at!(great);
                                at!(great) = ak;
                                great -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
        do_sort(a, less, great, compare);
    } else {
        do_sort(a, less, great, compare);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_large_lists() {
        let mut v: Vec<i64> = (0..500).map(|i| (i * 7919) % 263).collect();
        let mut expected = v.clone();
        expected.sort();
        dart_sort(&mut v, |a, b| a - b);
        assert_eq!(v, expected);
    }
}
