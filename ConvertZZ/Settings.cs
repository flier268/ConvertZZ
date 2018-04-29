// To parse this JSON data, add NuGet 'Newtonsoft.Json' then do:
//
//    using ConvertZZ;
//
//    var settings = Settings.FromJson(jsonString);

namespace ConvertZZ
{
    using Newtonsoft.Json;
    using Newtonsoft.Json.Converters;
    using System.Collections.Generic;
    using System.Globalization;
    using System.IO;
    using System.Text;

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
            using (StreamWriter sw = new StreamWriter(_FilePath, false, Encoding.UTF8))
            {
                sw.Write(settings.ToJson());
                sw.Flush();
            }
        }
    }


    public partial class Settings
    {
        public Settings()
        {
            QuickStart = new QuickStart();
            UnicodeAddBom = false;
            RecognitionEncoding = true;
            Prompt = true;
            MaxLengthPreview = 16000;
            VocabularyCorrection = true;
            HotKey = new HotKey();
            FileConvert = new FileConvert();
            ShowBalloonTip = true;
        }
        /// <summary>
        /// 快速動作設定(輔助鍵+點擊)
        /// </summary>
        [JsonProperty("QuickStart")]
        public QuickStart QuickStart { get; set; }

        /// <summary>
        /// 加入BOM到Unicode檔頭
        /// </summary>
        [JsonProperty("UnicodeAddBOM")]
        public bool UnicodeAddBom { get; set; }
        /// <summary>
        /// 試圖自動辨識編碼
        /// </summary>
        [JsonProperty("RecognitionEncoding")]
        public bool RecognitionEncoding { get; set; }
        /// <summary>
        /// 轉換完成後做出提示
        /// </summary>
        [JsonProperty("Prompt")]
        public bool Prompt { get; set; }
        /// <summary>
        /// 預覽的最大長度(kb)
        /// </summary>
        [JsonProperty("MaxLengthPreview")]
        public int MaxLengthPreview { get; set; }
        /// <summary>
        /// 詞彙修正
        /// </summary>
        [JsonProperty("Vocabulary correction")]
        public bool VocabularyCorrection { get; set; }
        /// <summary>
        /// 快捷鍵
        /// </summary>
        [JsonProperty("HotKey")]
        public HotKey HotKey { get; set; }
        /// <summary>
        /// 檔案轉換
        /// </summary>
        [JsonProperty("FileConvert")]
        public FileConvert FileConvert { get; set; }
        /// <summary>
        /// 顯示器泡提示
        /// </summary>
        [JsonProperty("ShowBalloonTip")]
        public bool ShowBalloonTip { get; set; }
    }

    public partial class FileConvert
    {
        public FileConvert()
        {
            DefaultPath = "!";
            IgnoreType = "*.exe,*.dll,*.ocx,*.com,*.sys,*.vxd,*.ocx,*.drv,*.zip,*.z[0-9][0-9],*.rar,*.r[0-9][0-9],*.lha,*.lzh,*.ar?,*.cab,*.tar,*.gz,*.bin,*.img,*.bmp,*.gif,*.jp*g,*.tif*,*.png,*.pcx,*.psd,*.ico,*.hlp,*.chm,*.pdf,*.au,*.mid,*.wav,*.mp*,*.class,*.swf,*.pp?,*.doc,*.xl?,*.md?,*.db,*.r*m,*.ra,*.ape,*.avi,*.asf,*.wm*,*.og?,*.lnk,*.torrent";
            FixLabel = "*.htm*,*.shtm*,*.asp,*.apsx,*.php*,*.pl,*.cgi,*.js";
        }
        /// <summary>
        /// 預設路徑
        /// </summary>
        [JsonProperty("DefaultPath")]
        public string DefaultPath { get; set; }
        /// <summary>
        /// 忽略類型
        /// </summary>
        [JsonProperty("IgnoreType")]
        public string IgnoreType { get; set; }
        /// <summary>
        /// 修正檔案內文的編碼標籤
        /// </summary>
        [JsonProperty("FixLabel")]
        public string FixLabel { get; set; }
    }

    public partial class HotKey
    {
        public HotKey()
        {
            AutoCopy = true;
            AutoPaste = true;
            Feature1 = new Feature() { Action = "a1", Enable = false, Key = 0, Modift = new List<int>() };
            Feature2 = new Feature() { Action = "a2", Enable = false, Key = 0, Modift = new List<int>() };
            Feature3 = new Feature() { Action = "a3", Enable = false, Key = 0, Modift = new List<int>() };
            Feature4 = new Feature() { Action = "a4", Enable = false, Key = 0, Modift = new List<int>() };
        }
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
        public Feature()
        {
            Action = "";
            Enable = false;
            Key = 0;
            Modift = new List<int>();
        }
        [JsonProperty("Action")]
        public string Action { get; set; }

        [JsonProperty("Enable")]
        public bool Enable { get; set; }

        [JsonProperty("Key")]
        public int Key { get; set; }

        [JsonProperty("Modift")]
        public List<int> Modift { get; set; }
    }

    public partial class QuickStart
    {
        public QuickStart()
        {
            CtrlClick = "";
            CtrlDrop = "";
            AltClick = "";
            AltDrop = "";
            ShiftClick = "";
            ShiftDrop = "";
        }
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
        public static string ToJson(this Settings self) => JsonConvert.SerializeObject(self, Formatting.Indented, ConvertZZ.Converter.Settings);
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
