using System;

namespace ConvertZZ.Moudle
{
    public static class Extension_ID3
    {
        public static bool SetPropertiesValue(this TagLib.Tag tag, string key, string str)
        {
            try
            {
                switch (key)
                {
                    case "FirstArtist":
                        SetFirstInGroup(tag, "Performers", str);
                        break;
                    case "FirstAlbumArtist":
                        SetFirstInGroup(tag, "AlbumArtists", str);
                        break;
                    case "FirstAlbumArtistSort":
                        SetFirstInGroup(tag, "AlbumArtistsSort", str);
                        break;
                    case "FirstPerformer":
                        SetFirstInGroup(tag, "Performers", str);
                        break;
                    case "FirstPerformerSort":
                        SetFirstInGroup(tag, "PerformersSort", str);
                        break;
                    case "FirstComposerSort":
                        SetFirstInGroup(tag, "ComposersSort", str);
                        break;
                    case "FirstComposer":
                        SetFirstInGroup(tag, "Composers", str);
                        break;
                    case "FirstGenre":
                        SetFirstInGroup(tag, "Genres", str);
                        break;
                    case "JoinedArtists":
                        SetUnJoinGroup(tag, "Performers", str);
                        break;
                    case "JoinedAlbumArtists":
                        SetUnJoinGroup(tag, "AlbumArtists", str);
                        break;
                    case "JoinedPerformers":
                        SetUnJoinGroup(tag, "Performers", str);
                        break;
                    case "JoinedPerformersSort":
                        SetUnJoinGroup(tag, "PerformersSort", str);
                        break;
                    case "JoinedComposers":
                        SetUnJoinGroup(tag, "Composers", str);
                        break;
                    case "JoinedGenres":
                        SetUnJoinGroup(tag, "Genres", str);
                        break;
                    default:
                        tag.GetType().GetProperty(key).SetValue(tag, str, null);
                        break;
                }
                return true;
            }
            catch { return false; }
        }
        private static void SetFirstInGroup(object obj, string key, string str)
        {
            var p = obj.GetType().GetProperty(key);
            string[] values = (p.GetValue(key) as string[]);
            values[0] = str;
            p.SetValue(obj, values, null);
        }
        private static void SetUnJoinGroup(object obj, string key, string str)
        {
            var p = obj.GetType().GetProperty(key);
            p.SetValue(obj, str.Split(new string[] { "; " }, StringSplitOptions.RemoveEmptyEntries), null);
        }
    }
}
