use std::fmt::Display;
use std::sync::Arc;

use calamine::{Data, Dimensions, Range, Rows, SheetType, SheetVisible};
use pyo3::class::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use crate::CellValue;

#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
pub enum SheetTypeEnum {
    /// WorkSheet
    WorkSheet,
    /// DialogSheet
    DialogSheet,
    /// MacroSheet
    MacroSheet,
    /// ChartSheet
    ChartSheet,
    /// VBA module
    Vba,
}

impl Display for SheetTypeEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SheetTypeEnum.{:?}", self)
    }
}

impl From<SheetType> for SheetTypeEnum {
    fn from(value: SheetType) -> Self {
        match value {
            SheetType::WorkSheet => Self::WorkSheet,
            SheetType::DialogSheet => Self::DialogSheet,
            SheetType::MacroSheet => Self::MacroSheet,
            SheetType::ChartSheet => Self::ChartSheet,
            SheetType::Vba => Self::Vba,
        }
    }
}

#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
pub enum SheetVisibleEnum {
    /// Visible
    Visible,
    /// Hidden
    Hidden,
    /// The sheet is hidden and cannot be displayed using the user interface. It is supported only by Excel formats.
    VeryHidden,
}

impl Display for SheetVisibleEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SheetVisibleEnum.{:?}", self)
    }
}

impl From<SheetVisible> for SheetVisibleEnum {
    fn from(value: SheetVisible) -> Self {
        match value {
            SheetVisible::Visible => Self::Visible,
            SheetVisible::Hidden => Self::Hidden,
            SheetVisible::VeryHidden => Self::VeryHidden,
        }
    }
}

#[pyclass]
#[derive(Clone, PartialEq)]
pub struct SheetMetadata {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    typ: SheetTypeEnum,
    #[pyo3(get)]
    visible: SheetVisibleEnum,
}

#[pymethods]
impl SheetMetadata {
    // implementation of some methods for testing
    #[new]
    fn py_new(name: &str, typ: SheetTypeEnum, visible: SheetVisibleEnum) -> Self {
        SheetMetadata {
            name: name.to_string(),
            typ,
            visible,
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "SheetMetadata(name='{}', typ={}, visible={})",
            self.name, self.typ, self.visible
        ))
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp, py: Python<'_>) -> PyObject {
        match op {
            CompareOp::Eq => self.eq(other).into_py(py),
            CompareOp::Ne => self.ne(other).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

impl SheetMetadata {
    pub fn new(name: String, typ: SheetType, visible: SheetVisible) -> Self {
        let typ = SheetTypeEnum::from(typ);
        let visible = SheetVisibleEnum::from(visible);
        SheetMetadata { name, typ, visible }
    }
}

#[pyclass]
pub struct CalamineSheet {
    #[pyo3(get)]
    name: String,
    range: Arc<Range<Data>>,
    merged_cells: Arc<Option<Vec<Dimensions>>>,
}

impl CalamineSheet {
    pub fn new(name: String, range: Range<Data>, merge_cells: Option<Vec<Dimensions>>) -> Self {
        CalamineSheet {
            name,
            range: Arc::new(range),
            merged_cells: Arc::new(merge_cells),
        }
    }
}

#[pymethods]
impl CalamineSheet {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("CalamineSheet(name='{}')", self.name))
    }

    #[getter]
    fn height(&self) -> usize {
        self.range.height()
    }

    #[getter]
    fn width(&self) -> usize {
        self.range.width()
    }

    #[getter]
    fn total_height(&self) -> u32 {
        self.range.end().unwrap_or_default().0
    }

    #[getter]
    fn total_width(&self) -> u32 {
        self.range.end().unwrap_or_default().1
    }

    #[getter]
    fn start(&self) -> Option<(u32, u32)> {
        self.range.start()
    }

    #[getter]
    fn end(&self) -> Option<(u32, u32)> {
        self.range.end()
    }

    #[pyo3(signature = (skip_empty_area=true, nrows=None))]
    fn to_python(
        slf: PyRef<'_, Self>,
        skip_empty_area: bool,
        nrows: Option<u32>,
    ) -> PyResult<Bound<'_, PyList>> {
        let nrows = match nrows {
            Some(nrows) => nrows,
            None => slf.range.end().map_or(0, |end| end.0 + 1),
        };

        let range = if skip_empty_area || Some((0, 0)) == slf.range.start() {
            Arc::clone(&slf.range)
        } else if let Some(end) = slf.range.end() {
            Arc::new(slf.range.range(
                (0, 0),
                (if nrows > end.0 { end.0 } else { nrows - 1 }, end.1),
            ))
        } else {
            Arc::clone(&slf.range)
        };

        Ok(PyList::new_bound(
            slf.py(),
            range.rows().take(nrows as usize).map(|row| {
                PyList::new_bound(slf.py(), row.iter().map(<&Data as Into<CellValue>>::into))
            }),
        ))
    }

    #[getter]
    fn merged_cells(slf: PyRef<'_, Self>) -> PyResult<Option<Py<PyList>>> {
        let merged_cells = slf.merged_cells.as_ref().as_ref();

        match merged_cells {
            Some(cells) => {
                let py_list = PyList::empty_bound(slf.py());

                for dim in cells {
                    let py_tuple = PyTuple::new_bound(slf.py(), &[dim.start, dim.end]);
                    py_list.append(py_tuple)?;
                }
                Ok(Some(py_list.into()))
            }
            None => Ok(None), // If there are no merge cells, return None
        }
    }

    fn iter_rows(&self) -> CalamineCellIterator {
        CalamineCellIterator::from_range(Arc::clone(&self.range))
    }
}

#[pyclass]
pub struct CalamineCellIterator {
    position: u32,
    start: (u32, u32),
    empty_row: Vec<CellValue>,
    iter: Rows<'static, Data>,
    #[allow(dead_code)]
    range: Arc<Range<Data>>,
}

impl CalamineCellIterator {
    fn from_range(range: Arc<Range<Data>>) -> CalamineCellIterator {
        let empty_row = (0..range.width())
            .map(|_| CellValue::String("".to_string()))
            .collect();
        CalamineCellIterator {
            empty_row,
            position: 0,
            start: range.start().unwrap(),
            iter: unsafe {
                std::mem::transmute::<
                    calamine::Rows<'_, calamine::Data>,
                    calamine::Rows<'static, calamine::Data>,
                >(range.rows())
            },
            range,
        }
    }
}

#[pymethods]
impl CalamineCellIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<Bound<'_, PyList>> {
        slf.position += 1;
        if slf.position > slf.start.0 {
            slf.iter.next().map(|row| {
                PyList::new_bound(slf.py(), row.iter().map(<&Data as Into<CellValue>>::into))
            })
        } else {
            Some(PyList::new_bound(slf.py(), slf.empty_row.clone()))
        }
    }
}
