extends Node

## Entry point: builds the Sim, runs a few sim-years, prints the revision.
## Milestone 1 hello world; the biome overlay renderer arrives next.


func _ready() -> void:
	var sim: Sim = Sim.create(1)
	add_child(sim)
	sim.hello()
	sim.step(10)
	print("revision after 10 sim-years: ", sim.revision())
	sim.queue_free()
