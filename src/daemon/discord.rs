use std::time;
use std::time::SystemTime;

use discord_rich_presence::{DiscordIpc, new_client};
use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};

use crate::daemon::CurrentSong;

const DISCORD_CLIENT_ID: &str = "927041178103332965";

pub struct Discord(Option<Box<dyn DiscordIpc>>);

pub fn discord_client() -> Discord {
    Discord((|| -> Option<Box<dyn DiscordIpc>> {
        let mut client = new_client(DISCORD_CLIENT_ID).ok()?;
        client.connect().ok()?;
        Some(Box::new(client))
    })())
}

pub fn set_discord_presence(Discord(discord): &mut Discord, song: &CurrentSong) {
    discord.as_mut().map(|discord| {
        let start = SystemTime::now() - song.elapsed;
        let start = start.duration_since(time::UNIX_EPOCH).unwrap();
        let mut activity = Activity::new()
            .details(song.metadata.artist.as_deref().unwrap_or("Unknown Artist"))
            .state(song.metadata.title.as_deref().unwrap_or("Unknown Title"))
            .timestamps(Timestamps::new().start(start.as_secs() as i64))
            .assets(Assets::new()
                .large_image("icon")
                .large_text("https://pmu.techno.fish/"));

        if let Some(origin) = &song.metadata.origin {
            let button = Button::new(&origin.name, &origin.link);
            activity = activity.buttons(vec![button]);
        }

        let _ = discord.set_activity(activity);
    });
}

pub fn clear_presence(Discord(discord): &mut Discord) {
    discord.as_mut().map(|discord| {
        let activity = Activity::new();
        let _ = discord.set_activity(activity);
    });
}
