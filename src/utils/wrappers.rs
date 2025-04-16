use std::{ops::{Deref, DerefMut}, str::FromStr};
use chrono::{DateTime as ChronoDatetime, Local};
use url::Url;
use crate::Result;

#[derive(Clone, Debug)]
pub struct W<T>(T);

impl<T> Deref for W<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for W<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type DateTime = W<ChronoDatetime<Local>>;

impl DateTime {
    pub fn now() -> Self {
        let now = Local::now();
        W(now)
    }
}

impl ToString for DateTime {
    fn to_string(&self) -> String {
        self.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

pub trait ToUrlStr {
    fn to_url(&self) -> Result<Url>;
}

impl ToUrlStr for str {
    fn to_url(&self) -> Result<Url> {
        let tmp = Url::from_str(self)?;
        Ok(tmp)
    }
}