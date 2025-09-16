use crate::*;

#[derive(Debug)]
/// FileSystem mount options
pub struct FSOptions<C: Clock> {
    pub(crate) clock: C,
    pub(crate) codepage: codepage::Codepage,
    pub(crate) update_file_fields: bool,
}

impl FSOptions<DefaultClock> {
    #[inline]
    /// Create a new options struct with the default options
    ///
    /// This is just an alias to [`Self::default`]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<C: Clock> FSOptions<C> {
    /// Set the codepage to be used by the filesystem
    pub fn set_codepage(&mut self, codepage: Codepage) {
        self.codepage = codepage
    }

    /// Set the codepage to be used by the filesystem (chainable)
    pub fn with_codepage(mut self, codepage: Codepage) -> Self {
        self.set_codepage(codepage);

        self
    }

    /// Whether to update the last accessed/modified file fields
    pub fn set_update_file_fields(&mut self, update: bool) {
        self.update_file_fields = update
    }

    /// Whether to update the last accessed/modified file fields (chainable)
    pub fn with_update_file_fields(mut self, update: bool) -> Self {
        self.update_file_fields = update;

        self
    }
}

impl Default for FSOptions<DefaultClock> {
    fn default() -> Self {
        Self {
            clock: DefaultClock,
            #[allow(clippy::default_constructed_unit_structs)] // the allow(clippy) is only used when no codepage feature is used
            codepage: codepage::Codepage::default(),
            update_file_fields: false,
        }
    }
}
