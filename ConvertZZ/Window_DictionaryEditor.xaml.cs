using ConvertZZ.Moudle;
using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;

namespace ConvertZZ
{
    /// <summary>
    /// Window_DictionaryEditor.xaml 的互動邏輯
    /// </summary>
    public partial class Window_DictionaryEditor : Window, INotifyPropertyChanged
    {
        public Window_DictionaryEditor()
        {
            DataContext = this;
            InitializeComponent();
            LoadDictionary();
        }
        private async void LoadDictionary()
        {
            await Task.Delay(0);
            DataGrid_ItemSource = App.ChineseConverter.Lines.Select(x => x.Clone()).ToList();
            FastReplace_Reload();
            DataGrid_Dictionary.ItemsSource = DataGrid_ItemSource;
            PerformCustomSort(true);
        }
        ChineseConverter ChineseConverter = new ChineseConverter();
        List<DictionaryFile_Helper.Line> DataGrid_ItemSource { get; set; }
        public string Input { get; set; } = "";

        public string Output { get; set; } = "";
        public bool C_to_T { get; set; } = true;
        void FastReplace_Reload()
        {
            ChineseConverter.Lines = DataGrid_ItemSource;
            ChineseConverter.Reload();
        }
        void InputToOutput()
        {
            Output = ChineseConverter.Convert(Input, C_to_T);
            if (C_to_T)
                Output = ChineseConverter.ToTraditional(Output);
            else
                Output = ChineseConverter.ToSimplified(Output);
        }

        private void DataGrid_CellEditEnding(object sender, DataGridCellEditEndingEventArgs e)
        {
            if (e.EditAction == DataGridEditAction.Commit)
            {
                var column = e.Column as DataGridBoundColumn;
                if (column != null)
                {
                    var bindingPath = (column.Binding as Binding).Path.Path;
                    int rowIndex = e.Row.GetIndex();
                    switch (bindingPath)
                    {
                        case "Enable":
                            (DataGrid_Dictionary.Items[rowIndex] as DictionaryFile_Helper.Line).Enable = (bool)(e.EditingElement as CheckBox).IsChecked;
                            break;
                        default:
                            var obj = (DataGrid_Dictionary.Items[rowIndex] as DictionaryFile_Helper.Line);
                            var propertyInfo = obj.GetType().GetProperty(bindingPath);
                            if (propertyInfo.PropertyType == typeof(int))
                                propertyInfo.SetValue(obj, int.Parse((e.EditingElement as TextBox).Text));
                            else
                                propertyInfo.SetValue(obj, (e.EditingElement as TextBox).Text);
                            break;
                    }
                }
            }
            FastReplace_Reload();
            InputToOutput();
        }
        #region Filter
        string DictionaryFilter_Text = "";
        private async void TextBox_Search_TextChanged(object sender, TextChangedEventArgs e)
        {
            TextBox tb = (TextBox)sender;
            string text = tb.Text;

            await Task.Delay(300);
            if (text == tb.Text)
            {
                CollectionView view = (CollectionView)CollectionViewSource.GetDefaultView(DataGrid_ItemSource);
                DictionaryFilter_Text = (e.Source as TextBox).Text;
                view.Filter = DictionaryFilter;
            }
        }
        private bool DictionaryFilter(object item)
        {
            if (string.IsNullOrEmpty(DictionaryFilter_Text))
                return true;
            else
            {
                var _ = (item as DictionaryFile_Helper.Line);
                return _.SimplifiedChinese.IndexOf(DictionaryFilter_Text, StringComparison.OrdinalIgnoreCase) >= 0 ||
                       _.TraditionalChinese.IndexOf(DictionaryFilter_Text, StringComparison.OrdinalIgnoreCase) >= 0 ||
                       _.Type.IndexOf(DictionaryFilter_Text, StringComparison.OrdinalIgnoreCase) >= 0;
            }
        }
        #endregion Filter
        #region Binding
        public event PropertyChangedEventHandler PropertyChanged;


        #endregion Binding
        private void AutoGeneratingColumn(object sender, DataGridAutoGeneratingColumnEventArgs e)
        {
            switch (e.PropertyName)
            {
                case "TraditionalChinese":
                    e.Column.Header = "繁體";
                    break;
                case "SimplifiedChinese":
                    e.Column.Header = "簡體";
                    break;
                case "TraditionalChinese_Priority":
                    e.Column.Header = "繁體優先度";
                    break;
                case "SimplifiedChinese_Priority":
                    e.Column.Header = "簡體優先度";
                    break;
                case "Enable":
                    e.Column.Header = "啟用";
                    break;
                case "Type":
                    e.Column.Header = "字典";
                    break;
                case "SimplifiedChinese_Length":
                case "TraditionalChinese_Length":
                    e.Cancel = true;
                    break;
            }
        }

        private async void Button_Reset_CheckedAsync(object sender, RoutedEventArgs e)
        {
            await Task.Delay(0);
            DataGrid_ItemSource = App.ChineseConverter.Lines.Select(x => x.Clone()).ToList();
            DataGrid_Dictionary.ItemsSource = DataGrid_ItemSource;
            ChineseConverter.Lines = DataGrid_ItemSource;
            ChineseConverter.Reload();
            InputToOutput();
            PerformCustomSort(true);
        }
        private async void Button_Save_CheckedAsync(object sender, RoutedEventArgs e)
        {
            if (Window_MessageBoxEx.ShowDialog("注意！此操作將不可逆，請先做好備份(Dictionary.csv)", "儲存字典?", "儲存", "取消") == Window_MessageBoxEx.MessageBoxExResult.A)
            {
                await DictionaryFile_Helper.Save(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary.csv"), DataGrid_ItemSource);
                await App.ChineseConverter.Load(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary.csv"));
                new Toast("已儲存!").Show();
            }
        }
        #region Sort
        private async void ToggleButton_CheckedAsync(object sender, RoutedEventArgs e)
        {
            await Task.Delay(0);
            if (DataGrid_Dictionary.ItemsSource == null)
                return;
            InputToOutput();
            PerformCustomSort(C_to_T);
        }
        private void PerformCustomSort(bool C_to_T)
        {
            ListCollectionView lcv = (ListCollectionView)CollectionViewSource.GetDefaultView(DataGrid_Dictionary.ItemsSource);
            MySort mySort = new MySort(C_to_T);
            lcv.CustomSort = mySort;
        }

        public class MySort : IComparer
        {
            bool C_To_T { get; set; }
            public MySort(bool C_To_T)
            {
                this.C_To_T = C_To_T;
            }
            int IComparer.Compare(object X, object Y)
            {
                var a = X as DictionaryFile_Helper.Line;
                var b = Y as DictionaryFile_Helper.Line;
                int CompareResoult;
                if (C_To_T)
                {
                    CompareResoult = b.SimplifiedChinese_Priority.CompareTo(a.SimplifiedChinese_Priority);
                    if (CompareResoult == 0)
                    {
                        CompareResoult = b.SimplifiedChinese_Length.CompareTo(a.SimplifiedChinese_Length);
                        if (CompareResoult == 0)
                            CompareResoult = b.SimplifiedChinese.CompareTo(a.SimplifiedChinese);
                    }
                }
                else
                {
                    CompareResoult = b.TraditionalChinese_Priority.CompareTo(a.TraditionalChinese_Priority);
                    if (CompareResoult == 0)
                    {
                        CompareResoult = b.TraditionalChinese_Length.CompareTo(a.TraditionalChinese_Length);
                        if (CompareResoult == 0)
                            CompareResoult = b.TraditionalChinese.CompareTo(a.TraditionalChinese);
                    }
                }
                return CompareResoult;
            }
        }
        #endregion Sort


        private void TextBox_TextChanged(object sender, TextChangedEventArgs e)
        {
            if (ChineseConverter == null) return;
            string value = (e.Source as TextBox).Text;
            Input = value.Substring(0, Math.Min(500, value.Length));
            InputToOutput();
        }

        private void textBox_PreviewExecuted(object sender, ExecutedRoutedEventArgs e)
        {
            if (e.Command == ApplicationCommands.Paste)
            {
                e.Handled = true;
                string text = ClipBoardHelper.GetClipBoard_UnicodeText();
                Input = text.Substring(0, Math.Min(500, text.Length));
            }
        }
    }
}
