pub trait SplitOn
where
	Self: Iterator,
	Self: Sized,
	Self::Item: PartialEq,
{
	/// Returns an iterator that returns Vec<I::Item>s that containing groups, split on the given seperator.
	/// If `inclusive` is set, then the returned group contains the seperator.
	fn split_on(self, seperator: Self::Item, inclusive: bool) -> SplitOnIter<Self>;

	/// Returns an iterator that returns Vec<I::Item>s that containing groups, split on the given seperator, including the seperator.
	fn split_on_inclusive(self, seperator: Self::Item) -> SplitOnIter<Self> {
		self.split_on(seperator, true)
	}

	/// Returns an iterator that returns Vec<I::Item>s that containing groups, split on the given seperator, excluding the seperator.
	fn split_on_exclusive(self, seperator: Self::Item) -> SplitOnIter<Self> {
		self.split_on(seperator, false)
	}
}

impl<I: Iterator + Sized> SplitOn for I
where
	I::Item: PartialEq,
{
	fn split_on(self, seperator: Self::Item, inclusive: bool) -> SplitOnIter<Self> {
		SplitOnIter::new(self, seperator, inclusive)
	}
}

pub struct SplitOnIter<I: Iterator>
where
	I::Item: PartialEq,
{
	iter: I,
	seperator: I::Item,
	inclusive: bool,
}

impl<I: Iterator> SplitOnIter<I>
where
	I::Item: PartialEq,
{
	pub fn new(iter: I, seperator: I::Item, inclusive: bool) -> SplitOnIter<I> {
		SplitOnIter {
			iter,
			seperator,
			inclusive,
		}
	}
}

impl<I: Iterator> Iterator for SplitOnIter<I>
where
	I::Item: PartialEq,
{
	type Item = Vec<I::Item>;

	fn next(&mut self) -> Option<Vec<I::Item>> {
		let mut values = Vec::new();
		for val in self.iter.by_ref() {
			if val == self.seperator {
				if self.inclusive {
					values.push(val);
				}

				return Some(values);
			}

			values.push(val);
		}

		if values.is_empty() {
			None
		} else {
			Some(values)
		}
	}
}
