using System;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using ConvertZZ.ViewModels;
using Fanhuaji_API.Class;
using Fanhuaji_API.Enum;

namespace ConvertZZ.Views
{
    /// <summary>
    /// Window_FanhuajiSetting.xaml 的互動邏輯
    /// </summary>
    public partial class Window_FanhuajiSetting : Window
    {
        private int replaceDicIndex = 0;
        private FanhuajiSettingViewModel ViewModel { get; }

        public Window_FanhuajiSetting()
        {
            InitializeComponent();
            ViewModel = (FanhuajiSettingViewModel)DataContext;
            string[] names = Enum.GetNames(typeof(Enum_Modules));
            foreach (string s in names)
            {
                if ((from x in App.Settings.Fanhuaji_Setting.Modules
                     where x.ModuleName.ToString() == s
                     select x).Count() == 0)
                {
                    App.Settings.Fanhuaji_Setting.Modules.Add(new Module((Enum_Modules)Enum.Parse(typeof(Enum_Modules), s), (bool?)null));
                }
            }
            ViewModel.Converter_T_to_S_Index = App.Settings.Fanhuaji_Setting.Converter_T_to_S_Index;
            ViewModel.Converter_S_to_T_Index = App.Settings.Fanhuaji_Setting.Converter_S_to_T_Index;
            ViewModel.IgnoreTextStyles = App.Settings.Fanhuaji_Setting.IgnoreTextStyles;
            ViewModel.JpTextStyles = App.Settings.Fanhuaji_Setting.JpTextStyles;
            ViewModel.CleanUpText = App.Settings.Fanhuaji_Setting.CleanUpText;
            ViewModel.EnsureNewlineAtEof = App.Settings.Fanhuaji_Setting.EnsureNewlineAtEof;
            ViewModel.TrimTrailingWhiteSpaces = App.Settings.Fanhuaji_Setting.TrimTrailingWhiteSpaces;
            ViewModel.UnifyLeadingHyphen = App.Settings.Fanhuaji_Setting.UnifyLeadingHyphen;
            ViewModel.TranslateTabsToSpaces_Index = App.Settings.Fanhuaji_Setting.TranslateTabsToSpaces_Index;
            ViewModel.JpStyleConversionStrategy_Index = App.Settings.Fanhuaji_Setting.JpStyleConversionStrategy_Index;
            ViewModel.JpTextConversionStrategy_Index = App.Settings.Fanhuaji_Setting.JpTextConversionStrategy_Index;
            ViewModel.UserPreReplace = new(App.Settings.Fanhuaji_Setting.UserPreReplace);
            ViewModel.UserPostReplace = new(App.Settings.Fanhuaji_Setting.UserPostReplace);
            ViewModel.UserProtectReplace = new(App.Settings.Fanhuaji_Setting.UserProtectReplace);

            ViewModel.Modules = new(App.Settings.Fanhuaji_Setting.Modules);
        }

        private void DataGrid_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            if (e.OriginalSource is Border border && sender is DataGrid dataGrid && !(dataGrid.CurrentColumn.DependencyObjectType.Name != "DataGridCheckBoxColumn"))
            {
                if (border.BindingGroup.Items[0] is Module module)
                {
                    if (module.Enable.HasValue == false)
                    {
                        module.Enable = false;
                    }
                    else if (module.Enable == false)
                    {
                        module.Enable = true;
                    }
                    else
                    {
                        module.Enable = null;
                    }
                }
            }
        }

        private void Window_Closing(object sender, CancelEventArgs e)
        {
            App.Settings.Fanhuaji_Setting.Converter_T_to_S_Index = ViewModel.Converter_T_to_S_Index;
            App.Settings.Fanhuaji_Setting.Converter_S_to_T_Index = ViewModel.Converter_S_to_T_Index;
            App.Settings.Fanhuaji_Setting.IgnoreTextStyles = ViewModel.IgnoreTextStyles;
            App.Settings.Fanhuaji_Setting.JpTextStyles = ViewModel.JpTextStyles;
            App.Settings.Fanhuaji_Setting.CleanUpText = ViewModel.CleanUpText;
            App.Settings.Fanhuaji_Setting.EnsureNewlineAtEof = ViewModel.EnsureNewlineAtEof;
            App.Settings.Fanhuaji_Setting.TrimTrailingWhiteSpaces = ViewModel.TrimTrailingWhiteSpaces;
            App.Settings.Fanhuaji_Setting.UnifyLeadingHyphen = ViewModel.UnifyLeadingHyphen;
            App.Settings.Fanhuaji_Setting.TranslateTabsToSpaces_Index = ViewModel.TranslateTabsToSpaces_Index;
            App.Settings.Fanhuaji_Setting.JpStyleConversionStrategy_Index = ViewModel.JpStyleConversionStrategy_Index;
            App.Settings.Fanhuaji_Setting.JpTextConversionStrategy_Index = ViewModel.JpTextConversionStrategy_Index;
            App.Settings.Fanhuaji_Setting.Modules = ViewModel.Modules?.ToList();
            App.Settings.Fanhuaji_Setting.UserPreReplace = ViewModel.UserPreReplace?.ToList();
            App.Settings.Fanhuaji_Setting.UserPostReplace = ViewModel.UserPostReplace?.ToList();
            App.Settings.Fanhuaji_Setting.UserProtectReplace = ViewModel.UserProtectReplace?.ToList();
            App.Save();
        }

        private void DataGrid_ReplaceList_AutoGeneratingColumn(object sender, DataGridAutoGeneratingColumnEventArgs e)
        {
            if (e.PropertyName == "Key")
            {
                e.Column.Header = "搜尋";
            }
            else
            {
                e.Column.Header = "取代";
            }
        }
    }
}