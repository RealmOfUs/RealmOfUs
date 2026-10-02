## Drive the real worker pipeline from tests that disable the viewer process loop.
extends RefCounted


static func drain(viewer: Node3D) -> bool:
	var deadline := Time.get_ticks_msec() + 30000
	while viewer.streaming_busy():
		viewer.pump_streaming()
		if Time.get_ticks_msec() > deadline:
			return false
		await viewer.get_tree().process_frame
	return true
