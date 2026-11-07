use macroquad::prelude::*;
fn conf()->Conf { Conf {window_title:"Art pixels".into(),high_dpi:true,
    window_width:800,window_height:500,sample_count:1,..Default::default()} }
#[macroquad::main(conf)]
async fn main() {
    let mut image=Image::gen_image_color(160,90,Color::from_rgba(15,26,51,255));
    for y in 60..90 { for x in 0..160 {
        image.set_pixel(x,y,Color::from_rgba(29,48,80,255));
    }}
    image.set_pixel(80,58,Color::from_rgba(226,182,121,255));
    image.set_pixel(81,58,Color::from_rgba(226,182,121,255));
    image.set_pixel(80,59,Color::from_rgba(226,182,121,255));
    image.set_pixel(81,59,Color::from_rgba(226,182,121,255));
    let texture=Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    loop {
        clear_background(BLACK);
        let dpi=screen_dpi_scale();
        let physical_w=(screen_width()*dpi).round();
        let physical_h=(screen_height()*dpi).round();
        let x=((physical_w-320.0)/2.0).floor()/dpi;
        let y=((physical_h-180.0)/2.0).floor()/dpi;
        draw_texture_ex(&texture,x,y,WHITE,DrawTextureParams {
            dest_size:Some(vec2(320.0/dpi,180.0/dpi)),..Default::default()
        });
        next_frame().await;
    }
}
