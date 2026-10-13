extends Node2D
## Plug in several keyboards: each one gets its own square. WASD or the arrow
## keys move your keyboard's square, and the other keyboards' squares stay put.
## Close the window or press Escape on the keyboard of your choice to quit.

const SPEED := 220.0
const COLORS := [
	Color.CORNFLOWER_BLUE, Color.ORANGE, Color.LIME_GREEN,
	Color.HOT_PINK, Color.GOLD, Color.TOMATO, Color.MEDIUM_PURPLE,
]

var manykey: ManyKeyboard
var players := {}  # keyboard_id -> {name, color, pos, held}

func _ready() -> void:
	manykey = ManyKeyboard.new()
	# Linux can't limit capture to the focused window; capture everything there.
	manykey.global_capture = OS.get_name() == "Linux"
	manykey.keyboard_added.connect(_on_keyboard_added)
	manykey.keyboard_removed.connect(_on_keyboard_removed)
	manykey.key_event.connect(_on_key_event)
	add_child(manykey)
	if not manykey.is_running():
		push_error(manykey.get_last_error())

func _on_keyboard_added(id: int, kb_name: String, _vendor: int, _product: int, _location: String) -> void:
	if players.has(id):
		players[id].connected = true
		return
	var n := players.size()
	players[id] = {
		"name": kb_name,
		"color": COLORS[n % COLORS.size()],
		"pos": Vector2(120 + 140 * (n % 6), 140 + 120 * (n / 6)),
		"held": {},
		"connected": true,
	}

func _on_keyboard_removed(id: int) -> void:
	if players.has(id):
		players[id].connected = false
		players[id].held.clear()

func _on_key_event(id: int, keycode: int, pressed: bool, echo: bool, _hid: int, _synth: bool, _native: int) -> void:
	if not players.has(id):
		return
	if pressed and keycode == KEY_ESCAPE:
		get_tree().quit()
	if not echo:
		players[id].held[keycode] = pressed

func _process(delta: float) -> void:
	for p in players.values():
		var dir := Vector2.ZERO
		var held: Dictionary = p.held
		if held.get(KEY_A, false) or held.get(KEY_LEFT, false): dir.x -= 1
		if held.get(KEY_D, false) or held.get(KEY_RIGHT, false): dir.x += 1
		if held.get(KEY_W, false) or held.get(KEY_UP, false): dir.y -= 1
		if held.get(KEY_S, false) or held.get(KEY_DOWN, false): dir.y += 1
		p.pos += dir.normalized() * SPEED * delta
		p.pos = p.pos.clamp(Vector2(20, 20), Vector2(940, 520))
	queue_redraw()

func _draw() -> void:
	var font := ThemeDB.fallback_font
	if players.is_empty():
		draw_string(font, Vector2(24, 40), "Waiting for keyboards... press a key.", HORIZONTAL_ALIGNMENT_LEFT, -1, 22)
	var y := 28.0
	for id in players:
		var p = players[id]
		var c: Color = p.color
		if not p.connected:
			c = c.darkened(0.6)
		draw_rect(Rect2(p.pos - Vector2(18, 18), Vector2(36, 36)), c)
		draw_string(font, p.pos + Vector2(-18, 38), "#%d" % id, HORIZONTAL_ALIGNMENT_LEFT, -1, 14)
		draw_string(font, Vector2(600, y), "#%d  %s%s" % [id, p.name, "" if p.connected else "  (disconnected)"], HORIZONTAL_ALIGNMENT_LEFT, -1, 14, c)
		y += 20.0
