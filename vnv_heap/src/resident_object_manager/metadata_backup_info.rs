/// A small wrapper for offset of the metadata backup of a resident object
#[derive(Clone, Copy)]
pub(crate) struct MetadataBackupInfo {
    offset: usize,
}

impl MetadataBackupInfo {
    const NONE_ELEMENT: usize = usize::MAX;

    #[inline]
    pub(crate) const fn empty() -> Self {
        Self {
            offset: MetadataBackupInfo::NONE_ELEMENT,
        }
    }

    #[inline]
    pub(crate) fn set(&mut self, offset: usize) {
        debug_assert_ne!(offset, MetadataBackupInfo::NONE_ELEMENT);
        self.offset = offset;
    }

    #[inline]
    pub(crate) fn unset(&mut self) {
        self.offset = Self::NONE_ELEMENT;
    }

    #[inline]
    pub(crate) fn get(&self) -> Option<usize> {
        if self.offset == MetadataBackupInfo::NONE_ELEMENT {
            None
        } else {
            Some(self.offset)
        }
    }
}
