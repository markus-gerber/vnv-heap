/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use core::{cell::RefCell, marker::PhantomData};

use crate::{
    allocation_identifier::AllocationIdentifier, modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    }, vnv_heap::VNVHeapInner, vnv_list_mut_ref::VNVListMutRef, vnv_list_ref::VNVListRef
};

pub(crate) struct ListItemContainer<T> {
    pub(crate) prev: AllocationIdentifier<ListItemContainer<T>>,
    pub(crate) next: AllocationIdentifier<ListItemContainer<T>>,
    pub(crate) data: T,
}

pub struct VNVList<
    'a,
    'b: 'a,
    T: Sized + Clone,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>,
    head: AllocationIdentifier<ListItemContainer<T>>,
    tail: AllocationIdentifier<ListItemContainer<T>>,
    phantom_data: PhantomData<T>,
}

impl<
        'a,
        'b: 'a,
        T: Sized + Clone,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVList<'a, 'b, T, A, N, M>
{
    pub(crate) fn new(vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>) -> Self {
        Self {
            vnv_heap,
            head: AllocationIdentifier::new_invalid(),
            tail: AllocationIdentifier::new_invalid(),
            phantom_data: PhantomData,
        }
    }

    pub fn push_front(&mut self, data: T) -> Result<(), ()> {
        let item = ListItemContainer {
            data,
            next: self.head.clone(),
            prev: AllocationIdentifier::new_invalid()
        };

        let mut heap = self.vnv_heap.borrow_mut();
        
        let new_id = unsafe { heap.allocate(item, false).unwrap() };

        if !self.head.is_invalid() {
            let prev_obj = unsafe {
                let tmp = match heap.get_mut(&self.head, false) {
                    Ok(tmp) => tmp,
                    Err(()) => {
                        heap.deallocate(&new_id, false).unwrap();
                        return Err(());
                    }
                };

                tmp.as_mut().unwrap()
            };

            prev_obj.prev = new_id.clone();
            unsafe { heap.release_mut(&self.head) };
        } else {
            self.tail = new_id.clone();
        }

        self.head = new_id;

        return Ok(())
    }

    pub fn pop_back(&mut self) -> Result<Option<T>, ()> {
        if self.tail.is_invalid() {
            // no elements left
            return Ok(None);
        }

        let mut heap = self.vnv_heap.borrow_mut();

        let (data, prev) = unsafe {
            let item = heap.get_mut(&self.tail, false)?;
            let item = item.as_mut().unwrap();
            debug_assert!(item.next.is_invalid());

            (item.data.clone(), item.prev.clone())
        };

        unsafe { heap.release_mut(&self.tail) };


        if !prev.is_invalid() {
            let prev_obj = unsafe {
                let tmp = heap.get_mut(&prev, false)?;
                tmp.as_mut().unwrap()
            };

            prev_obj.next = AllocationIdentifier::new_invalid();
            unsafe { heap.release_mut(&prev) };
        } else {
            // this was the last item in the list, its empty now
            debug_assert_eq!(self.head.offset, self.tail.offset);
            self.head = AllocationIdentifier::new_invalid();
        }

        unsafe {
            // TODO: handle this error somehow
            // we now would be in a invalid state
            heap.deallocate(&self.tail, false).expect("invalid state");
        }

        self.tail = prev;

        return Ok(Some(data))
    }

    pub fn peek_back(&mut self) -> Result<Option<VNVListRef<'a, '_, '_, 'b, T, A, N, M>>, ()> {
        if self.tail.is_invalid() {
            // no elements in list
            return Ok(None);
        }

        let mut heap = self.vnv_heap.borrow_mut();

        let item = unsafe {
            let tmp = heap.get_ref(&self.tail, false)?;
            tmp.as_ref().unwrap()
        };

        Ok(Some(unsafe { VNVListRef::new(self.vnv_heap, &self.tail, item) }))
    }

    pub fn peek_back_mut(&mut self) -> Result<Option<VNVListMutRef<'a, '_, '_, 'b, T, A, N, M>>, ()> {
        if self.tail.is_invalid() {
            // no elements in list
            return Ok(None);
        }

        let mut heap = self.vnv_heap.borrow_mut();

        let item = unsafe {
            let tmp = heap.get_mut(&self.tail, false)?;
            tmp.as_mut().unwrap()
        };

        Ok(Some(unsafe { VNVListMutRef::new(self.vnv_heap, &self.tail, item) }))
    }

}

impl<T: Sized + Clone, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Drop
    for VNVList<'_, '_, T, A, N, M>
{
    fn drop(&mut self) {
        while let Some(_) = self.pop_back().unwrap() {
            // nothing todo, we just drop all values
        }

        debug_assert!(self.head.is_invalid());
        debug_assert!(self.tail.is_invalid());
    }
}

/*
pub struct VNVListIterMut<
    'a,
    'b: 'a,
    'c,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    list: &'c mut VNVList<'a, 'b, T, A, N, M>,

    curr_head: AllocationIdentifier<ListItemContainer<T>>,
    curr_tail: AllocationIdentifier<ListItemContainer<T>>,
}

impl<
        'a,
        'b: 'a,
        'c,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVListIterMut<'a, 'b, 'c, T, A, N, M>
{
    pub fn next<'d>(&mut self) -> Option<VNVListItem<'a, 'b, 'c, 'd, T, A, N, M>> {
        todo!();
    }
}

pub struct VNVListItem<
    'a,
    'b: 'a,
    'c,
    'd,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    iter: &'d VNVListIterMut<'a, 'b, 'c, T, A, N, M>
}

*/

#[cfg(test)]
mod test {
    use std::collections::VecDeque;

    use crate::test::get_test_heap;

    #[test]
    fn test_list_1() {
        let mut buffer = [0u8; 1024];
        let heap = get_test_heap("test_list_1", 4 * 1024, &mut buffer, 1024, |_, _| {});

        let mut list = heap.new_list::<u64>();
        let mut check_list: VecDeque<u64> = VecDeque::new();

        macro_rules! check_integrity {
            () => {
                {
                    if check_list.len() == 0 {
                        assert!(list.peek_back().unwrap().is_none());
                        assert!(list.peek_back_mut().unwrap().is_none());
                
                        assert!(list.pop_back().unwrap().is_none());                
                    } else {
                        let x = list.peek_back().unwrap().unwrap();
                        assert_eq!(check_list[check_list.len() - 1], *x);
                    }
                }
            };
        }

        macro_rules! push {
            ($item: expr) => {
                list.push_front($item).unwrap();
                check_list.push_front($item);
                check_integrity!();
            };
        }

        macro_rules! pop {
            () => {
                if let Some(item) = list.pop_back().unwrap() {
                    assert_eq!(item, check_list.pop_back().unwrap());
                } else {
                    assert!(check_list.is_empty());
                }
                check_integrity!();
            };
        }

        check_integrity!();
        pop!();

        push!(23);
        pop!();
        pop!();

        push!(1);
        push!(5);
        push!(93);
        push!(2);
        push!(7);

        pop!();

        push!(9);

        pop!();
        pop!();
        pop!();
        pop!();
        pop!();
        pop!();


    }
}