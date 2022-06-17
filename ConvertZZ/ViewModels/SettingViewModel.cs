using System.Collections.Generic;
using System.Linq;
using CommunityToolkit.Mvvm.Messaging;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Core.Messages;
using PropertyChanged;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    internal class SettingViewModel
    {
        public SettingViewModel()
        {
            LoadSetting();
            Loaded = true;
        }

        private void LoadSetting()
        {
            AssistiveTouchEnable = App.Settings.AssistiveTouch;
            VocabularyCorrenctionEnable = App.Settings.VocabularyCorrection;
            UseLocalDic = App.Settings.Engine == EEngine.Local;
            UseFanhuajiAPI = App.Settings.Engine == EEngine.Fanhuaji;
            PromptEnable = App.Settings.Prompt;
            RecognitionEncodingEnable = App.Settings.RecognitionEncoding;
            MaxPriviewLength = App.Settings.MaxLengthPreview;
            CheckVersion = App.Settings.CheckVersion;

            DefaultPath = App.Settings.FileConvert.DefaultPath;
            FixLabel = App.Settings.FileConvert.FixLabel;
            TypeFilter = App.Settings.FileConvert.TypeFilter;
            UnicodeAddBom = App.Settings.FileConvert.UnicodeAddBom;
            Quick_L1 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftClick_Ctrl));
            Quick_L2 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftClick_Alt));
            Quick_L3 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftClick_Shift));
            Quick_L4 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftDrop_Ctrl));
            Quick_L5 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftDrop_Alt));
            Quick_L6 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.LeftDrop_Shift));
            Quick_R1 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightClick_Ctrl));
            Quick_R2 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightClick_Alt));
            Quick_R3 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightClick_Shift));
            Quick_R4 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightDrop_Ctrl));
            Quick_R5 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightDrop_Alt));
            Quick_R6 = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.QuickStart.RightDrop_Shift));

            AutoCopy = App.Settings.HotKey.AutoCopy;
            AutoPaste = App.Settings.HotKey.AutoPaste;
            ShortCut1_IsActived = App.Settings.HotKey.Feature1.Enable;
            ShortCut1_Action = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.HotKey.Feature1.Command));
            ShortCut1_Key = App.Settings.HotKey.Feature1.Key;
            ShortCut1_ModifyKey = App.Settings.HotKey.Feature1.Modift;
            ShortCut2_IsActived = App.Settings.HotKey.Feature2.Enable;
            ShortCut2_Action = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.HotKey.Feature2.Command));
            ShortCut2_Key = App.Settings.HotKey.Feature2.Key;
            ShortCut2_ModifyKey = App.Settings.HotKey.Feature2.Modift;
            ShortCut3_IsActived = App.Settings.HotKey.Feature3.Enable;
            ShortCut3_Action = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.HotKey.Feature3.Command));
            ShortCut3_Key = App.Settings.HotKey.Feature3.Key;
            ShortCut3_ModifyKey = App.Settings.HotKey.Feature3.Modift;
            ShortCut4_IsActived = App.Settings.HotKey.Feature4.Enable;
            ShortCut4_Action = Action.SingleOrDefault(x => x.Value.Equals(App.Settings.HotKey.Feature4.Command));
            ShortCut4_Key = App.Settings.HotKey.Feature4.Key;
            ShortCut4_ModifyKey = App.Settings.HotKey.Feature4.Modift;
        }

        private void SaveSetting()
        {
            if (Loaded == false)
                return;
            App.Settings.AssistiveTouch = AssistiveTouchEnable;
            App.Settings.VocabularyCorrection = VocabularyCorrenctionEnable;
            App.Settings.Engine = UseLocalDic ? EEngine.Local : EEngine.Fanhuaji;
            App.Settings.Prompt = PromptEnable;
            App.Settings.RecognitionEncoding = RecognitionEncodingEnable;
            App.Settings.MaxLengthPreview = MaxPriviewLength;
            App.Settings.CheckVersion = CheckVersion;

            App.Settings.FileConvert.DefaultPath = DefaultPath;
            App.Settings.FileConvert.FixLabel = FixLabel;
            App.Settings.FileConvert.TypeFilter = TypeFilter;
            App.Settings.FileConvert.UnicodeAddBom = UnicodeAddBom;

            App.Settings.QuickStart.LeftClick_Ctrl = Quick_L1.Value;
            App.Settings.QuickStart.LeftClick_Alt = Quick_L2.Value;
            App.Settings.QuickStart.LeftClick_Shift = Quick_L3.Value;
            App.Settings.QuickStart.LeftDrop_Ctrl = Quick_L4.Value;
            App.Settings.QuickStart.LeftDrop_Alt = Quick_L5.Value;
            App.Settings.QuickStart.LeftDrop_Shift = Quick_L6.Value;
            App.Settings.QuickStart.RightClick_Ctrl = Quick_R1.Value;
            App.Settings.QuickStart.RightClick_Alt = Quick_R2.Value;
            App.Settings.QuickStart.RightClick_Shift = Quick_R3.Value;
            App.Settings.QuickStart.RightDrop_Ctrl = Quick_R4.Value;
            App.Settings.QuickStart.RightDrop_Alt = Quick_R5.Value;
            App.Settings.QuickStart.RightDrop_Shift = Quick_R6.Value;

            App.Settings.HotKey.AutoCopy = AutoCopy;
            App.Settings.HotKey.AutoPaste = AutoPaste;
            App.Settings.HotKey.Feature1.Enable = ShortCut1_IsActived;
            App.Settings.HotKey.Feature1.Command = ShortCut1_Action.Value;
            App.Settings.HotKey.Feature1.Key = ShortCut1_Key;
            App.Settings.HotKey.Feature1.Modift = ShortCut1_ModifyKey;
            App.Settings.HotKey.Feature2.Enable = ShortCut2_IsActived;
            App.Settings.HotKey.Feature2.Command = ShortCut2_Action.Value;
            App.Settings.HotKey.Feature2.Key = ShortCut2_Key;
            App.Settings.HotKey.Feature2.Modift = ShortCut2_ModifyKey;
            App.Settings.HotKey.Feature3.Enable = ShortCut3_IsActived;
            App.Settings.HotKey.Feature3.Command = ShortCut3_Action.Value;
            App.Settings.HotKey.Feature3.Key = ShortCut3_Key;
            App.Settings.HotKey.Feature3.Modift = ShortCut3_ModifyKey;
            App.Settings.HotKey.Feature4.Enable = ShortCut4_IsActived;
            App.Settings.HotKey.Feature4.Command = ShortCut4_Action.Value;
            App.Settings.HotKey.Feature4.Key = ShortCut4_Key;
            App.Settings.HotKey.Feature4.Modift = ShortCut4_ModifyKey;

            App.Save();
        }

        private void InverseMainWindowIsVisible()
        {
            WeakReferenceMessenger.Default.Send(new InvokeCommandMessage(), new EnumAdapter<EnumCommand>(EnumCommand.ShowHideWindowCommand));
        }

        public static Dictionary<string, CompleteCommand> Action { get; } = new()
        {
            { "無", new(EnumCommand.None, null)},
            { "隱藏/顯示懸浮球", new(EnumCommand.ShowHideWindowCommand, null)},
            { "GBK>Big5", new(EnumCommand.GbkToBig5Command, null)},
            { "Big5>GBK", new(EnumCommand.Big5ToGbkCommand, null)},
            { "Unicode簡>Unicode繁", new(EnumCommand.Unicode簡ToUnicode繁Command, null)},
            { "Unicode繁>Unicode簡", new(EnumCommand.Unicode繁ToUnicode簡Command,null)},
            { "Unicode>Html Code十進制", new(EnumCommand.UnicodeToHtmlDexCodeCommand, null)},
            { "Unicode>Html Code十六進制", new(EnumCommand.UnicodeToHtmlHexCodeCommand, null)},
            { "HTML Code>Unicode", new(EnumCommand.HtmlCodeToUnicodeCommand, null)},
            { "Unicode>GBK", new(EnumCommand.CommonEncodingConvertCommand, "Unicode>GBK")},
            { "Unicode>Big5", new(EnumCommand.CommonEncodingConvertCommand, "Unicode>Big5")},
            { "Unicode>Shift-JIS", new(EnumCommand.CommonEncodingConvertCommand, "Unicode>Shift-JIS")},
            { "GBK>Unicode", new(EnumCommand.CommonEncodingConvertCommand, "GBK>Unicode")},
            { "Big5>Unicode", new(EnumCommand.CommonEncodingConvertCommand, "Big5>Unicode")},
            { "Shift-JIS>Unicode", new(EnumCommand.CommonEncodingConvertCommand, "Shift-JIS>Unicode")},
            { "Shift-JIS>GBK", new(EnumCommand.CommonEncodingConvertCommand, "Shift-JIS>GBK")},
            { "Shift-JIS>Big5", new(EnumCommand.CommonEncodingConvertCommand, "Shift-JIS>Big5")},
            { "GBK>Shift-JIS", new(EnumCommand.CommonEncodingConvertCommand, "GBK>Shift-JIS")},
            { "Big5>Shift-JIS", new(EnumCommand.CommonEncodingConvertCommand, "Big5>Shift-JIS")},
            { "HZ>GBK", new(EnumCommand.CommonEncodingConvertCommand, "hz-gb-2312>GBK")},
            { "HZ>Big5", new(EnumCommand.CommonEncodingConvertCommand, "hz-gb-2312>Big5")},
            { "GBK>HZ", new(EnumCommand.CommonEncodingConvertCommand, "GBK>hz-gb-2312")},
            { "Big5>HZ", new(EnumCommand.CommonEncodingConvertCommand, "Big5>hz-gb-2312")},
            { "半形>全形", new(EnumCommand.HalfSymbolToFullSizeSymbolCommand, null)},
            { "全形>半形", new(EnumCommand.FullSizeSymbolToHalfSymbolCommand, null)}
        };

        [OnChangedMethod(nameof(SaveSetting))]
        [OnChangedMethod(nameof(InverseMainWindowIsVisible))]
        public bool AssistiveTouchEnable { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool VocabularyCorrenctionEnable { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool PromptEnable { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool RecognitionEncodingEnable { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public int MaxPriviewLength { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool CheckVersion { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L1 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L2 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L3 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L4 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L5 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_L6 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R1 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R2 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R3 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R4 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R5 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> Quick_R6 { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? DefaultPath { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? TypeFilter { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? FixLabel { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool UseLocalDic { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool UseFanhuajiAPI { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool AutoCopy { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool AutoPaste { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut1_Key { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut1_ModifyKey { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut2_Key { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut2_ModifyKey { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut3_Key { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut3_ModifyKey { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut4_Key { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public string? ShortCut4_ModifyKey { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool ShortCut1_IsActived { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool ShortCut2_IsActived { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool ShortCut3_IsActived { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool ShortCut4_IsActived { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> ShortCut1_Action { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> ShortCut2_Action { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> ShortCut3_Action { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public KeyValuePair<string, CompleteCommand> ShortCut4_Action { get; set; }

        [OnChangedMethod(nameof(SaveSetting))]
        public bool UnicodeAddBom { get; set; }

        public bool Loaded { get; }
    }
}