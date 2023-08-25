use chrono::{Local, DateTime};
use epics_ca::{request, types::{EpicsEnum, EpicsString, EpicsTimeStamp}};


#[derive(Debug)]
pub enum RawValue {
    // Scalar
    Char(request::Time<u8>),
    Short(request::Time<i16>),
    Long(request::Time<i32>),
    Enum(request::Time<EpicsEnum>),
    Float(request::Time<f32>),
    Double(request::Time<f64>),
    String(request::Time<EpicsString>),
    // Arrays
    ShortArray(Box<request::Time<[i16]>>),
    LongArray(Box<request::Time<[i32]>>),
    FloatArray(Box<request::Time<[f32]>>),
    DoubleArray(Box<request::Time<[f64]>>),
    StringArray(Box<request::Time<[EpicsString]>>),
}

macro_rules! impl_get_stamp {
    ($op:ident, $( $name:ident ),+) => {
        match $op {
            $(RawValue::$name(val) => val.stamp,)+
        }
    };
}

impl RawValue {
    pub fn get_stamp(&self) -> EpicsTimeStamp {
        impl_get_stamp!(
            self,
            Char,
            Short,
            Long,
            Float,
            Double,
            Enum,
            String,
            ShortArray,
            LongArray,
            FloatArray,
            DoubleArray,
            StringArray
        )
    }

    pub fn format_scalar(&self) -> String {
        match self {
            RawValue::Short(val) => format!("{}", val.value),
            RawValue::Long(val) => format!("{}", val.value),
            RawValue::Float(val) => format!("{:.5}", val.value),
            RawValue::Double(val) => format!("{:.5}", val.value),
            RawValue::Enum(val) => format!("{}", val.value.0),
            RawValue::String(val) => val.value.to_string_lossy().to_string(),
            _ => format!("<formatting not implemented yet for {self:#?}>"),
        }
    }

    pub fn format_array(&self, padding: usize) -> String {
        fn format_array<T>(padding: usize, data: &request::Time<[T]>) -> String
        where
            T: ToString,
            [T]: epics_ca::types::Value,
        {
            let mut rest: Vec<_> = data.value.iter().map(|d| d.to_string()).collect();
            for _ in 0..(padding - rest.len()) {
                rest.push("0".into());
            }
            rest.join(" ").to_string()
        }

        match self {
            RawValue::LongArray(val) => format_array(padding, val),
            _ => format!("<formatting not implemented yet for {self:#?}>"),
        }
    }
}

#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub elements: usize,
    pub value: RawValue,
}

impl Info {
    pub fn new(name: String, elements: usize, value: RawValue) -> Self {
        Info {
            name,
            elements,
            value,
        }
    }

    pub fn is_scalar(&self) -> bool {
        self.elements == 1
    }

    pub fn format_scalar(&self) -> String {
        self.value.format_scalar()
    }

    pub fn format_array(&self, count: usize) -> String {
        self.value.format_array(count)
    }

    pub fn format_stamp(&self) -> String {
        let stamp: DateTime<Local> = self.value.get_stamp().to_system().into();
        format!("{}", stamp.format("%F %T%.6f"))
    }
}
