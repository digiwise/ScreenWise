// Standalone diagnostics only. No production admission decision or captured content.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::c_void,
    time::{Duration, Instant},
};
use windows::{
    core::{Interface, HRESULT},
    Win32::{
        Foundation::HWND,
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
                SAFEARRAY,
            },
            Ole::{
                SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetLBound,
                SafeArrayGetUBound, SafeArrayUnaccessData,
            },
        },
        UI::Accessibility::*,
    },
};

// Offline self-test only; reuse oleaut32 without enabling another crate feature.
#[link(name = "oleaut32")]
extern "system" {
    fn SafeArrayCreateVector(vt: u16, lower: i32, count: u32) -> *mut SAFEARRAY;
}

unsafe fn array_key(array: *const SAFEARRAY) -> windows::core::Result<Option<Vec<i32>>> {
    if array.is_null() {
        return Ok(None);
    }
    if SafeArrayGetDim(array) != 1 || (*array).cbElements != 4 {
        return Ok(None);
    }
    let lo = SafeArrayGetLBound(array, 1)?;
    let hi = SafeArrayGetUBound(array, 1)?;
    let len = i64::from(hi) - i64::from(lo) + 1;
    if !(1..=128).contains(&len) {
        return Ok(None);
    }
    let mut data: *mut c_void = std::ptr::null_mut();
    SafeArrayAccessData(array, &mut data)?;
    let key = if data.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(data.cast::<i32>(), len as usize).to_vec())
    };
    SafeArrayUnaccessData(array)?;
    Ok(key)
}
unsafe fn direct_key(element: &IUIAutomationElement) -> windows::core::Result<Option<Vec<i32>>> {
    let array = element.GetRuntimeId()?;
    let result = array_key(array);
    if !array.is_null() {
        SafeArrayDestroy(array)?;
    }
    result
}
fn check(deadline: Instant) -> windows::core::Result<()> {
    if Instant::now() >= deadline {
        Err(windows::core::Error::from_hresult(HRESULT(
            0x800705b4u32 as i32,
        )))
    } else {
        Ok(())
    }
}
#[derive(Default)]
struct Stats {
    nodes: u64,
    missing: u64,
    direct_ok: u64,
    direct_missing: u64,
    direct_errors: u64,
    cache_direct_equal: u64,
    cache_direct_different: u64,
    direct_repeat_equal: u64,
    direct_repeat_different: u64,
    duplicate_ids: u64,
    direct_us: u128,
    direct_max_us: u128,
    direct_calls: u64,
    fallback_total_us: u128,
    fallback_max_us: u128,
    variants: BTreeMap<u16, u64>,
    errors: BTreeMap<i32, u64>,
    kinds: BTreeMap<(i32, u16), u64>,
    ids: HashSet<Vec<i32>>,
    missing_control_flags: BTreeMap<i32, u64>,
}
impl Stats {
    unsafe fn inspect(
        &mut self,
        e: &IUIAutomationElement,
        deadline: Instant,
        repeat: bool,
    ) -> windows::core::Result<()> {
        check(deadline)?;
        let variant = e.GetCachedPropertyValueEx(UIA_RuntimeIdPropertyId, true)?;
        let raw = variant.as_raw();
        let vt = raw.Anonymous.Anonymous.vt;
        *self.variants.entry(vt).or_default() += 1;
        let control = i32::try_from(&e.GetCachedPropertyValueEx(UIA_ControlTypePropertyId, true)?)
            .unwrap_or(-1);
        *self.kinds.entry((control, vt)).or_default() += 1;
        // windows-core's VARIANT uses its internal SAFEARRAY binding; the Win32
        // SAFEARRAY has the same Windows ABI layout and is required by Ole APIs.
        let cached = if vt == 8195 {
            array_key(raw.Anonymous.Anonymous.Anonymous.parray.cast::<SAFEARRAY>())?
        } else {
            None
        };
        self.nodes += 1;
        if cached.is_none() {
            self.missing += 1;
            let flag =
                bool::try_from(&e.GetCachedPropertyValueEx(UIA_IsControlElementPropertyId, true)?)
                    .map(|b| b as i32)
                    .unwrap_or(-1);
            *self.missing_control_flags.entry(flag).or_default() += 1;
        }
        check(deadline)?;
        let start = Instant::now();
        let direct = direct_key(e);
        let us = start.elapsed().as_micros();
        self.direct_us += us;
        self.direct_max_us = self.direct_max_us.max(us);
        self.direct_calls += 1;
        if cached.is_none() {
            self.fallback_total_us += us;
            self.fallback_max_us = self.fallback_max_us.max(us);
        }
        check(deadline)?;
        match direct {
            Ok(Some(key)) => {
                self.direct_ok += 1;
                if let Some(c) = &cached {
                    if *c == key {
                        self.cache_direct_equal += 1;
                    } else {
                        self.cache_direct_different += 1;
                    }
                }
                if !self.ids.insert(key.clone()) {
                    self.duplicate_ids += 1;
                }
                if repeat && cached.is_none() {
                    check(deadline)?;
                    let start = Instant::now();
                    let again = direct_key(e);
                    let us = start.elapsed().as_micros();
                    self.direct_us += us;
                    self.direct_max_us = self.direct_max_us.max(us);
                    self.direct_calls += 1;
                    match again {
                        Ok(Some(other)) if other == key => self.direct_repeat_equal += 1,
                        Ok(_) => self.direct_repeat_different += 1,
                        Err(err) => {
                            self.direct_errors += 1;
                            *self.errors.entry(err.code().0).or_default() += 1;
                        }
                    }
                }
            }
            Ok(None) => self.direct_missing += 1,
            Err(err) => {
                self.direct_errors += 1;
                *self.errors.entry(err.code().0).or_default() += 1;
            }
        }
        check(deadline)
    }
    fn print(&self, phase: u8, elapsed: u128, complete: bool, error: i32) {
        let variants = self
            .variants
            .iter()
            .map(|(a, b)| format!("[{a},{b}]"))
            .collect::<Vec<_>>()
            .join(",");
        let kinds = self
            .kinds
            .iter()
            .map(|((a, b), c)| format!("[{a},{b},{c}]"))
            .collect::<Vec<_>>()
            .join(",");
        let errors = self
            .errors
            .iter()
            .map(|(a, b)| format!("[{a},{b}]"))
            .collect::<Vec<_>>()
            .join(",");
        let flags = self
            .missing_control_flags
            .iter()
            .map(|(a, b)| format!("[{a},{b}]"))
            .collect::<Vec<_>>()
            .join(",");
        println!("{{\"phase\":{phase},\"complete\":{},\"elapsed_us\":{elapsed},\"hresult\":{error},\"nodes\":{},\"cached_id_missing\":{},\"direct_id_ok\":{},\"direct_id_missing\":{},\"direct_errors\":{},\"cached_direct_equal\":{},\"cached_direct_different\":{},\"direct_repeat_equal\":{},\"direct_repeat_different\":{},\"duplicate_direct_ids\":{},\"direct_total_us\":{},\"direct_max_us\":{},\"variant_counts\":[{variants}],\"control_variant_counts\":[{kinds}],\"missing_id_control_flags\":[{flags}],\"direct_hresult_counts\":[{errors}]}}",complete as u8,self.nodes,self.missing,self.direct_ok,self.direct_missing,self.direct_errors,self.cache_direct_equal,self.cache_direct_different,self.direct_repeat_equal,self.direct_repeat_different,self.duplicate_ids,self.direct_us,self.direct_max_us);
        println!("{{\"phase\":{phase},\"direct_calls\":{},\"fallback_total_us\":{},\"fallback_max_us\":{}}}",self.direct_calls,self.fallback_total_us,self.fallback_max_us);
    }
}
unsafe fn cache(
    a: &IUIAutomation,
    scope: TreeScope,
    raw: bool,
) -> windows::core::Result<IUIAutomationCacheRequest> {
    let c = a.CreateCacheRequest()?;
    c.SetTreeScope(scope)?;
    c.SetAutomationElementMode(AutomationElementMode_Full)?;
    c.SetTreeFilter(&if raw {
        a.RawViewCondition()?
    } else {
        a.ControlViewCondition()?
    })?;
    for p in [
        UIA_RuntimeIdPropertyId,
        UIA_ControlTypePropertyId,
        UIA_IsControlElementPropertyId,
    ] {
        c.AddProperty(p)?;
    }
    Ok(c)
}
unsafe fn run(hwnd: isize) -> windows::core::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let timeouts: IUIAutomation2 = CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL)?;
    timeouts.SetConnectionTimeout(500)?;
    timeouts.SetTransactionTimeout(500)?;
    let a: IUIAutomation = timeouts.cast()?;
    let root = a.ElementFromHandle(HWND(hwnd as *mut c_void))?;
    let mut sets = HashMap::new();
    // Obtain the failing query's answer before a large hierarchy can exhaust the budget.
    for phase in [1u8, 2, 0] {
        let start = Instant::now();
        let mut stats = Stats::default();
        let result = (|| -> windows::core::Result<()> {
            check(deadline)?;
            if phase == 0 {
                let updated = root.BuildUpdatedCache(&cache(&a, TreeScope_Subtree, false)?)?;
                let mut pending = vec![updated];
                while let Some(e) = pending.pop() {
                    if stats.nodes >= 5000 {
                        return Err(windows::core::Error::from_hresult(HRESULT(
                            0x80070057u32 as i32,
                        )));
                    }
                    stats.inspect(&e, deadline, false)?;
                    let mut ptr: *mut c_void = std::ptr::null_mut();
                    (Interface::vtable(&e).GetCachedChildren)(Interface::as_raw(&e), &mut ptr)
                        .ok()?;
                    if !ptr.is_null() {
                        let children = IUIAutomationElementArray::from_raw(ptr);
                        for i in 0..children.Length()? {
                            pending.push(children.GetElement(i)?);
                        }
                    }
                }
            } else {
                let d = a.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &windows::core::VARIANT::from(UIA_DocumentControlTypeId.0),
                )?;
                let e = a.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &windows::core::VARIANT::from(UIA_EditControlTypeId.0),
                )?;
                let c = a.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &windows::core::VARIANT::from(UIA_ComboBoxControlTypeId.0),
                )?;
                let condition = a.CreateOrCondition(&d, &a.CreateOrCondition(&e, &c)?)?;
                let found = root.FindAllBuildCache(
                    TreeScope_Subtree,
                    &condition,
                    &cache(&a, TreeScope_Element, phase == 2)?,
                )?;
                let n = found.Length()?;
                if !(0..=5000).contains(&n) {
                    return Err(windows::core::Error::from_hresult(HRESULT(
                        0x80070057u32 as i32,
                    )));
                }
                for i in 0..n {
                    stats.inspect(&found.GetElement(i)?, deadline, true)?;
                }
            }
            Ok(())
        })();
        stats.print(
            phase,
            start.elapsed().as_micros(),
            result.is_ok(),
            result.as_ref().err().map(|e| e.code().0).unwrap_or(0),
        );
        sets.insert(phase, stats.ids);
        if result.is_err() {
            return Ok(());
        }
    }
    println!("{{\"phase\":3,\"query_ids_outside_hierarchy\":{},\"raw_query_ids_outside_hierarchy\":{},\"query_filter_id_difference\":{}}}",sets[&1].difference(&sets[&0]).count(),sets[&2].difference(&sets[&0]).count(),sets[&1].symmetric_difference(&sets[&2]).count());
    Ok(())
}

#[derive(Default)]
struct AcquisitionStats {
    nodes: usize,
    id_missing: usize,
    required_missing: usize,
    property_types: BTreeMap<(i32, i32, u16), usize>,
    acquisitions: usize,
    acquisition_us: u128,
    decode_us: u128,
    first_reject_us: Option<u128>,
    id_set: HashSet<Vec<i32>>,
    duplicate_ids: usize,
    query_outside_hierarchy: usize,
}
fn property_needed(control: i32, property: UIA_PROPERTY_ID) -> bool {
    if property == UIA_NamePropertyId {
        control == UIA_WindowControlTypeId.0 || control == UIA_DocumentControlTypeId.0
    } else if property == UIA_ValueValuePropertyId {
        control == UIA_DocumentControlTypeId.0
            || control == UIA_EditControlTypeId.0
            || control == UIA_ComboBoxControlTypeId.0
    } else {
        true
    }
}
unsafe fn acquisition_cache(
    a: &IUIAutomation,
    scope: TreeScope,
    properties: &[UIA_PROPERTY_ID],
) -> windows::core::Result<IUIAutomationCacheRequest> {
    let c = a.CreateCacheRequest()?;
    c.SetTreeScope(scope)?;
    c.SetTreeFilter(&a.ControlViewCondition()?)?;
    c.SetAutomationElementMode(AutomationElementMode_Full)?;
    for property in properties {
        c.AddProperty(*property)?;
    }
    Ok(c)
}
unsafe fn inspect_properties(
    e: &IUIAutomationElement,
    properties: &[UIA_PROPERTY_ID],
    stats: &mut AcquisitionStats,
    deadline: Instant,
    start: Instant,
    hierarchy: bool,
) -> windows::core::Result<()> {
    check(deadline)?;
    let control =
        i32::try_from(&e.GetCachedPropertyValueEx(UIA_ControlTypePropertyId, true)?).unwrap_or(-1);
    let mut key = None;
    for property in properties {
        if !property_needed(control, *property) {
            continue;
        }
        let value = e.GetCachedPropertyValueEx(*property, true)?;
        let raw = value.as_raw();
        let vt = raw.Anonymous.Anonymous.vt;
        *stats
            .property_types
            .entry((control, property.0, vt))
            .or_default() += 1;
        if *property == UIA_RuntimeIdPropertyId {
            key = if vt == 8195 {
                array_key(raw.Anonymous.Anonymous.Anonymous.parray.cast::<SAFEARRAY>())?
            } else {
                None
            };
            if key.is_none() {
                stats.id_missing += 1;
                stats
                    .first_reject_us
                    .get_or_insert(start.elapsed().as_micros());
            }
        } else if *property == UIA_ControlTypePropertyId
            && (vt != 3 || !(50000..=50040).contains(&control))
        {
            stats.required_missing += 1;
        } else if (*property == UIA_NamePropertyId
            || (*property == UIA_ValueValuePropertyId && control == UIA_DocumentControlTypeId.0))
            && vt != 8
        {
            stats.required_missing += 1;
        } else if *property == UIA_IsPasswordPropertyId && ![0, 11, 13].contains(&vt) {
            stats.required_missing += 1;
        } else if *property == UIA_ValueValuePropertyId && ![0, 8, 13].contains(&vt) {
            stats.required_missing += 1;
        }
    }
    if hierarchy {
        stats.nodes += 1;
        if let Some(key) = key {
            if !stats.id_set.insert(key) {
                stats.duplicate_ids += 1;
            }
        }
    } else if let Some(key) = key {
        if !stats.id_set.contains(&key) {
            stats.query_outside_hierarchy += 1;
        }
    }
    check(deadline)
}
unsafe fn inspect_hierarchy(
    root: IUIAutomationElement,
    props: &[UIA_PROPERTY_ID],
    stats: &mut AcquisitionStats,
    deadline: Instant,
    start: Instant,
) -> windows::core::Result<()> {
    let mut pending = vec![(root, 0usize)];
    while let Some((element, depth)) = pending.pop() {
        check(deadline)?;
        if stats.nodes >= 5000 || depth > 35 {
            return Err(windows::core::Error::from_hresult(HRESULT(
                0x80070057u32 as i32,
            )));
        }
        inspect_properties(&element, props, stats, deadline, start, true)?;
        let mut ptr: *mut c_void = std::ptr::null_mut();
        (Interface::vtable(&element).GetCachedChildren)(Interface::as_raw(&element), &mut ptr)
            .ok()?;
        if !ptr.is_null() {
            let children = IUIAutomationElementArray::from_raw(ptr);
            for i in 0..children.Length()? {
                pending.push((children.GetElement(i)?, depth + 1));
            }
        }
    }
    Ok(())
}
unsafe fn compare_acquisition(hwnd: isize, pass: u32) -> windows::core::Result<()> {
    // New setup/root for every mode; no trees, identities or results cross modes.
    for mode in if pass % 2 == 0 { [0u8, 1] } else { [1u8, 0] } {
        let mode_start = Instant::now();
        let timeouts: IUIAutomation2 = CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL)?;
        let a: IUIAutomation = timeouts.cast()?;
        timeouts.SetConnectionTimeout(500)?;
        timeouts.SetTransactionTimeout(500)?;
        let root = a.ElementFromHandle(HWND(hwnd as *mut c_void))?;
        let setup_us = mode_start.elapsed().as_micros();
        let start = Instant::now();
        let deadline = start + Duration::from_secs(10);
        let mut stats = AcquisitionStats::default();
        let structural = [
            UIA_RuntimeIdPropertyId,
            UIA_ControlTypePropertyId,
            UIA_IsPasswordPropertyId,
        ];
        let full = [
            UIA_RuntimeIdPropertyId,
            UIA_ControlTypePropertyId,
            UIA_IsPasswordPropertyId,
            UIA_NamePropertyId,
            UIA_ValueValuePropertyId,
        ];
        let result = (|| -> windows::core::Result<()> {
            let props = if mode == 1 {
                &full[..]
            } else {
                &structural[..]
            };
            let cache = acquisition_cache(&a, TreeScope_Subtree, props)?;
            check(deadline)?;
            let t = Instant::now();
            let tree = root.BuildUpdatedCache(&cache)?;
            stats.acquisitions += 1;
            stats.acquisition_us += t.elapsed().as_micros();
            check(deadline)?;
            if mode == 0 {
                // Match production order: acquire scoped properties before hierarchy decoding.
                let mut query_results = Vec::new();
                for value_query in [true, false] {
                    let document = a.CreatePropertyCondition(
                        UIA_ControlTypePropertyId,
                        &windows::core::VARIANT::from(UIA_DocumentControlTypeId.0),
                    )?;
                    let condition = if value_query {
                        let edit = a.CreatePropertyCondition(
                            UIA_ControlTypePropertyId,
                            &windows::core::VARIANT::from(UIA_EditControlTypeId.0),
                        )?;
                        let combo = a.CreatePropertyCondition(
                            UIA_ControlTypePropertyId,
                            &windows::core::VARIANT::from(UIA_ComboBoxControlTypeId.0),
                        )?;
                        a.CreateOrCondition(&document, &a.CreateOrCondition(&edit, &combo)?)?
                    } else {
                        let window = a.CreatePropertyCondition(
                            UIA_ControlTypePropertyId,
                            &windows::core::VARIANT::from(UIA_WindowControlTypeId.0),
                        )?;
                        a.CreateOrCondition(&document, &window)?
                    };
                    let props = [
                        UIA_RuntimeIdPropertyId,
                        UIA_ControlTypePropertyId,
                        if value_query {
                            UIA_ValueValuePropertyId
                        } else {
                            UIA_NamePropertyId
                        },
                    ];
                    let cache = acquisition_cache(&a, TreeScope_Element, &props)?;
                    check(deadline)?;
                    let t = Instant::now();
                    let results = root.FindAllBuildCache(TreeScope_Subtree, &condition, &cache)?;
                    stats.acquisitions += 1;
                    stats.acquisition_us += t.elapsed().as_micros();
                    check(deadline)?;
                    let n = results.Length()?;
                    if !(0..=5000).contains(&n) {
                        return Err(windows::core::Error::from_hresult(HRESULT(
                            0x80070057u32 as i32,
                        )));
                    }
                    let t = Instant::now();
                    // Observe the first identity rejection in actual query acquisition order.
                    for i in 0..n {
                        check(deadline)?;
                        let e = results.GetElement(i)?;
                        let v = e.GetCachedPropertyValueEx(UIA_RuntimeIdPropertyId, true)?;
                        if v.as_raw().Anonymous.Anonymous.vt != 8195 {
                            stats
                                .first_reject_us
                                .get_or_insert(start.elapsed().as_micros());
                        }
                    }
                    stats.decode_us += t.elapsed().as_micros();
                    query_results.push((results, props));
                }
                let t = Instant::now();
                inspect_hierarchy(tree, &structural, &mut stats, deadline, start)?;
                stats.decode_us += t.elapsed().as_micros();
                for (results, props) in query_results {
                    let t = Instant::now();
                    for i in 0..results.Length()? {
                        inspect_properties(
                            &results.GetElement(i)?,
                            &props,
                            &mut stats,
                            deadline,
                            start,
                            false,
                        )?;
                    }
                    stats.decode_us += t.elapsed().as_micros();
                }
            } else {
                let t = Instant::now();
                inspect_hierarchy(tree, &full, &mut stats, deadline, start)?;
                stats.decode_us += t.elapsed().as_micros();
            }
            Ok(())
        })();
        let total = start.elapsed().as_micros();
        let types = stats
            .property_types
            .iter()
            .map(|((c, p, v), n)| format!("[{c},{p},{v},{n}]"))
            .collect::<Vec<_>>()
            .join(",");
        // No permission is produced: complete only means the diagnostic acquisition finished.
        println!("{{\"phase\":4,\"mode\":{mode},\"pass\":{pass},\"complete\":{},\"hresult\":{},\"setup_us\":{setup_us},\"total_us\":{total},\"acquisition_us\":{},\"decode_us\":{},\"acquisitions\":{},\"nodes\":{},\"missing_runtime_id_occurrences\":{},\"unsupported_required_property_occurrences\":{},\"duplicate_hierarchy_ids\":{},\"query_ids_outside_hierarchy\":{},\"first_identity_reject_us\":{},\"within_375ms\":{},\"within_750ms\":{},\"property_type_counts\":[{types}]}}",result.is_ok() as u8,result.as_ref().err().map(|e|e.code().0).unwrap_or(0),stats.acquisition_us,stats.decode_us,stats.acquisitions,stats.nodes,stats.id_missing,stats.required_missing,stats.duplicate_ids,stats.query_outside_hierarchy,stats.first_reject_us.map(|x|x.to_string()).unwrap_or_else(||"null".into()),((total+setup_us)<=375000) as u8,((total+setup_us)<=750000) as u8);
    }
    Ok(())
}
fn main() {
    let arg = std::env::args().nth(1);
    if arg.as_deref() == Some("--self-test") {
        assert!(check(Instant::now() - Duration::from_millis(1)).is_err());
        assert!(check(Instant::now() + Duration::from_secs(1)).is_ok());
        assert!(property_needed(
            UIA_DocumentControlTypeId.0,
            UIA_NamePropertyId
        ));
        assert!(property_needed(
            UIA_EditControlTypeId.0,
            UIA_ValueValuePropertyId
        ));
        assert!(!property_needed(
            UIA_ButtonControlTypeId.0,
            UIA_ValueValuePropertyId
        ));
        unsafe {
            assert_eq!(array_key(std::ptr::null()).unwrap(), None);
            let array = SafeArrayCreateVector(3, 7, 3);
            assert!(!array.is_null());
            let mut data: *mut c_void = std::ptr::null_mut();
            SafeArrayAccessData(array, &mut data).unwrap();
            std::ptr::copy_nonoverlapping([11i32, 22, 33].as_ptr(), data.cast::<i32>(), 3);
            SafeArrayUnaccessData(array).unwrap();
            assert_eq!(array_key(array).unwrap(), Some(vec![11, 22, 33]));
            SafeArrayDestroy(array).unwrap();
            let empty = SafeArrayCreateVector(3, 0, 0);
            assert!(!empty.is_null());
            assert_eq!(array_key(empty).unwrap(), None);
            SafeArrayDestroy(empty).unwrap();
        }
        println!("{{\"self_test_passed\":1}}");
        return;
    }
    let Some(hwnd) = arg
        .and_then(|s| s.parse::<isize>().ok())
        .filter(|h| *h != 0)
    else {
        println!("{{\"argument_error\":1}}");
        std::process::exit(2);
    };
    unsafe {
        if let Err(e) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            println!("{{\"com_hresult\":{}}}", e.code().0);
            return;
        }
        let comparison = std::env::args().nth(2).as_deref() == Some("--compare");
        let pass = std::env::args()
            .nth(3)
            .and_then(|x| x.parse::<u32>().ok())
            .unwrap_or(1);
        if let Err(e) = if comparison {
            compare_acquisition(hwnd, pass)
        } else {
            run(hwnd)
        } {
            println!("{{\"setup_hresult\":{}}}", e.code().0);
        }
        CoUninitialize();
    }
}
