// To parse this JSON data, add NuGet 'Newtonsoft.Json' then do:
//
//    using ConvertZZ;
//
//    var settings = Settings.FromJson(jsonString);

namespace ConvertZZ
{
    using System;
    using System.Collections.Generic;

    using System.Globalization;
    using System.IO;
    using System.Text;
    using Newtonsoft.Json;
    using Newtonsoft.Json.Converters;

    public partial class App
    {
        private static Settings settings = new Settings();
        static string _FilePath;

        public static Settings Settings { get => settings; set => settings = value; }

        public static void Reload(string FilePath)
        {
            if (File.Exists(FilePath))
            {
                _FilePath = FilePath;
                using (StreamReader streamReader = new StreamReader(FilePath, Encoding.UTF8))
                {
                    settings = Settings.FromJson(streamReader.ReadToEnd());
                }
            }
        }
        public static void Save()
        {
            using (StreamWriter sw = new StreamWriter(_FilePath,false,Encoding.UTF8))
            {
                sw.Write(settings.ToJson());
                sw.Flush();
            }
        }
    }


        public partial class Settings
    {
        [JsonProperty("QuickStart")]
        public QuickStart QuickStart { get; set; }

        [JsonProperty("UnicodeAddBOM")]
        public bool UnicodeAddBom { get; set; }

        [JsonProperty("RecognitionEncoding")]
        public bool RecognitionEncoding { get; set; }

        [JsonProperty("Prompt")]
        public bool Prompt { get; set; }

        [JsonProperty("MaxLengthPreview")]
        public long MaxLengthPreview { get; set; }

        [JsonProperty("Vocabulary correction")]
        public bool VocabularyCorrection { get; set; }

        [JsonProperty("HotKey")]
        public HotKey HotKey { get; set; }

        [JsonProperty("FileConvert")]
        public FileConvert FileConvert { get; set; }

    }

    public partial class FileConvert
    {
        [JsonProperty("DefaultPath")]
        public string DefaultPath { get; set; }

        [JsonProperty("IgnoreType")]
        public string IgnoreType { get; set; }

        [JsonProperty("FixLabel")]
        public string FixLabel { get; set; }
    }

    public partial class HotKey
    {
        [JsonProperty("AutoCopy")]
        public bool AutoCopy { get; set; }

        [JsonProperty("AutoPaste")]
        public bool AutoPaste { get; set; }

        [JsonProperty("Feature1")]
        public Feature Feature1 { get; set; }

        [JsonProperty("Feature2")]
        public Feature Feature2 { get; set; }

        [JsonProperty("Feature3")]
        public Feature Feature3 { get; set; }

        [JsonProperty("Feature4")]
        public Feature Feature4 { get; set; }
    }

    public partial class Feature
    {
        [JsonProperty("Action")]
        public string Action { get; set; }

        [JsonProperty("Enable")]
        public bool Enable { get; set; }

        [JsonProperty("Key")]
        public long Key { get; set; }

        [JsonProperty("Modift")]
        public List<long> Modift { get; set; }
    }

    public partial class QuickStart
    {
        [JsonProperty("CtrlClick")]
        public string CtrlClick { get; set; }

        [JsonProperty("AltClick")]
        public string AltClick { get; set; }

        [JsonProperty("ShiftClick")]
        public string ShiftClick { get; set; }

        [JsonProperty("CtrlDrop")]
        public string CtrlDrop { get; set; }

        [JsonProperty("AltDrop")]
        public string AltDrop { get; set; }

        [JsonProperty("ShiftDrop")]
        public string ShiftDrop { get; set; }
    }

    public partial class Settings
    {
        public static Settings FromJson(string json) => JsonConvert.DeserializeObject<Settings>(json, ConvertZZ.Converter.Settings);
    }

    public static class Serialize
    {
        public static string ToJson(this Settings self) => JsonConvert.SerializeObject(self,Formatting.Indented, ConvertZZ.Converter.Settings);
    }

    internal class Converter
    {
        public static readonly JsonSerializerSettings Settings = new JsonSerializerSettings
        {
            MetadataPropertyHandling = MetadataPropertyHandling.Ignore,
            DateParseHandling = DateParseHandling.None,
            Converters = {
                new IsoDateTimeConverter { DateTimeStyles = DateTimeStyles.AssumeUniversal }
            },
        };
    }
}
