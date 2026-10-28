extends SceneTree

## Temporary binding smoke test: cast a meteor through the gdext API and print
## the verse the people wrote about it.


func _initialize() -> void:
	var sim: Sim = Sim.create(1)
	root.add_child(sim)
	sim.step(300)
	var tile: Vector2i = sim.garden_tile()
	print("garden: ", tile, " people: ", sim.culture_name(sim.chosen_culture()))
	sim.cast_power(4, tile.x, tile.y)
	sim.step(1)
	print("witness: ", sim.newest_witness())
	for verse in sim.scripture(sim.chosen_culture(), 0, 3):
		print("verse: ", verse)
	quit()
