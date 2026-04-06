// cargo_apple-Bridging-Header.h

#ifndef cargo_apple_Bridging_Header_h
#define cargo_apple_Bridging_Header_h

void start_camera_capture(void);
int get_initial_frame_width(void);
int get_initial_frame_height(void);
int get_initial_frame_size(void);
int get_latest_frame_stride(void);
void *get_latest_frame(void);
char *check_camera_access(void);

#endif /* cargo_apple_Bridging_Header_h */
