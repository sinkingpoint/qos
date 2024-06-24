use std::fmt::Debug;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
	#[error("Cycle detected")]
	Cycle,
}

/// A Directed Acyclic Graph.
#[derive(PartialEq, Eq)]
pub struct Graph<E: PartialEq + Clone + Debug, V: PartialEq + Clone> {
	vertices: Vec<V>,
	edges: Vec<(V, V, E)>,
}

impl<E: PartialEq + Clone + Debug, V: PartialEq + Clone + Debug> Graph<E, V> {
	pub fn empty() -> Self {
		Self {
			vertices: Vec::new(),
			edges: Vec::new(),
		}
	}

	/// Adds a vertex to the graph.
	pub fn add_vertex(&mut self, vertex: V) {
		for v in &self.vertices {
			if v == &vertex {
				return;
			}
		}

		self.vertices.push(vertex);
	}

	/// Adds an edge to the graph, adding the vertices if they don't exist.
	pub fn add_edge(&mut self, from: V, edge: E, to: V) {
		if !self.vertices.contains(&from) {
			self.vertices.push(from.clone());
		}

		if !self.vertices.contains(&to) {
			self.vertices.push(to.clone());
		}

		self.edges.push((from, to, edge));
	}

	/// Flattens the graph into a list of vertices, in a topological order.
	/// Uses [Kahn's algorithm](https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm).
	pub fn flatten(&self) -> Result<Vec<V>, GraphError> {
		let mut visited = Vec::new();

		let mut edges = self.edges.clone();
		let mut stack: Vec<V> = self
			.vertices
			.iter()
			.filter(|&v| !edges.iter().any(|(_, to, _)| to == v))
			.cloned()
			.collect();

		while let Some(vertex) = stack.pop() {
			visited.push(vertex.clone());
			let mut target_nodes = Vec::new();
			edges.retain(|(from, to, _)| {
				let retain = from != &vertex;
				if !retain {
					target_nodes.push(to.clone());
				}

				retain
			});

			for v in target_nodes.iter().filter(|&v| !edges.iter().any(|(_, to, _)| to == v)) {
				stack.push(v.clone());
			}
		}

		if edges.is_empty() {
			Ok(visited)
		} else {
			Err(GraphError::Cycle)
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	#[test]
	fn test_linear_graph() {
		let mut graph = Graph::empty();
		graph.add_vertex("c");
		graph.add_vertex("b");
		graph.add_vertex("a");
		graph.add_edge("a", (), "b");
		graph.add_edge("b", (), "c");

		assert_eq!(graph.flatten().unwrap(), vec!["a", "b", "c"]);
	}

	#[test]
	fn test_diamond_graph() {
		let mut graph = Graph::empty();
		graph.add_vertex("d");
		graph.add_vertex("c");
		graph.add_vertex("b");
		graph.add_vertex("a");
		graph.add_edge("a", (), "b");
		graph.add_edge("a", (), "c");
		graph.add_edge("b", (), "d");
		graph.add_edge("c", (), "d");

		assert_eq!(graph.flatten().unwrap(), vec!["a", "c", "b", "d"]);
	}

	#[test]
	fn test_trivial_cycle_graph() {
		let mut graph = Graph::empty();
		graph.add_vertex("a");
		graph.add_vertex("b");
		graph.add_edge("a", (), "b");
		graph.add_edge("b", (), "a");

		assert!(graph.flatten().is_err());
	}

	#[test]
	fn test_cycle_graph() {
		let mut graph = Graph::empty();
		graph.add_vertex("a");
		graph.add_vertex("b");
		graph.add_vertex("c");
		graph.add_edge("a", (), "b");
		graph.add_edge("b", (), "c");
		graph.add_edge("c", (), "b");

		assert!(graph.flatten().is_err());
	}

	#[test]
	fn test_pushing_diamond_graph() {
		let mut graph = Graph::empty();
		graph.add_vertex("a");
		graph.add_vertex("b");
		graph.add_vertex("c");
		graph.add_vertex("d");
		graph.add_vertex("e");
		graph.add_edge("a", (), "b");
		graph.add_edge("a", (), "c");
		graph.add_edge("c", (), "d");
	}
}
