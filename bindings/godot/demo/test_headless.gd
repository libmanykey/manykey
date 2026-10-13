extends SceneTree
## Used by the maintainers' automated check: loads the extension in a real
## Godot, starts a ManyKeyboard with global capture, and prints what it hears.
## Usage: godot --headless --path demo --script res://test_headless.gd [-- --global]
## On a fresh checkout, import once first (godot --headless --path demo --import),
## or Godot won't know the ManyKeyboard class yet.
## Without --global it uses the default (focused-window-only) capture, which in a
## headless Godot with no window should hear no key events at all.
## The handlers call back into the node (known=, held=), as game scripts do.
## ci/linux-godot-smoke.py reads this output; keep its fields in step.

var mk: ManyKeyboard
var elapsed := 0.0
var reported := false

func _initialize() -> void:
	mk = ManyKeyboard.new()
	mk.global_capture = "--global" in OS.get_cmdline_user_args()
	mk.include_injected = true     # the check injects keys with SendInput
	mk.keyboard_added.connect(func(id, kb_name, vid, pid, _loc):
		print("ADDED id=%d name=%s vid=%d pid=%d known=%d" % [id, kb_name, vid, pid, mk.get_keyboards().size()]))
	mk.keyboard_removed.connect(func(id): print("REMOVED id=%d" % id))
	mk.key_event.connect(func(id, key, pressed, echo, hid, synth, native):
		print("KEY id=%d key=%d pressed=%s echo=%s hid=%d synth=%s native=%d held=%s" % [
			id, key, pressed, echo, hid, synth, native, mk.get_pressed_keys(id)]))
	root.add_child(mk)

func _process(delta: float) -> bool:
	# The node starts in its _ready, which runs after _initialize returns.
	if not reported:
		reported = true
		print("RUNNING=%s ERROR=[%s]" % [mk.is_running(), mk.get_last_error()])
	elapsed += delta
	if elapsed > 12.0:
		for kb in mk.get_keyboards():
			print("HELD id=%d keys=%s" % [kb.id, mk.get_pressed_keys(kb.id)])
		print("KEYBOARDS=%s" % [mk.get_keyboards()])
		print("DONE")
		return true
	return false
