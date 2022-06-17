using System.Collections.ObjectModel;
using Fanhuaji_API.Class;
using PropertyChanged;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    public class FanhuajiSettingViewModel
    {
        public int Converter_T_to_S_Index { get; set; }
        public int Converter_S_to_T_Index { get; set; }
        public string? IgnoreTextStyles { get; set; }
        public string? JpTextStyles { get; set; }
        public bool CleanUpText { get; set; }
        public bool EnsureNewlineAtEof { get; set; }
        public bool TrimTrailingWhiteSpaces { get; set; }
        public bool UnifyLeadingHyphen { get; set; }
        public int TranslateTabsToSpaces_Index { get; set; }
        public int JpStyleConversionStrategy_Index { get; set; }
        public int JpTextConversionStrategy_Index { get; set; }
        public ObservableCollection<Module>? Modules { get; set; }
        public ObservableCollection<KeyValue>? UserPreReplace { get; set; }
        public ObservableCollection<KeyValue>? UserPostReplace { get; set; }
        public ObservableCollection<KeyValue>? UserProtectReplace { get; set; }
    }
}