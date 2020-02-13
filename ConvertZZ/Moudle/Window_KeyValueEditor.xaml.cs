using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows;

namespace ConvertZZ.Moudle
{
    /// <summary>
    /// Window_KeyValueEditor.xaml 的互動邏輯
    /// </summary>
    public partial class Window_KeyValueEditor : Window, INotifyPropertyChanged
    {
        public Window_KeyValueEditor(Button button1, Button button2, ObservableCollection<KeyValueItem> keyValueItems)
        {
            InitializeComponent();
            DataContext = this;
            this.KeyValueItems = keyValueItems;
            this.button1_Content = button1.Content;
            this.button2_Content = button2.Content;
            Button1_Action = button1.Action;
            Button2_Action = button2.Action;
        }
        private void Button1_Click(object sender, RoutedEventArgs e)
        {
            Button1_Action?.Invoke();
        }
        private void Button2_Click(object sender, RoutedEventArgs e)
        {
            Button2_Action?.Invoke();
        }

        public class Button
        {
            public string Content { get; set; }
            public Action Action { get; set; }
        }
        public class KeyValueItem
        {
            public string Key { get; set; }
            public string Value { get; set; }
        }

        public ObservableCollection<KeyValueItem> KeyValueItems { get; set; }

        public string button1_Content { get; set; }

        public string button2_Content { get; set; }
        public Action Button1_Action { get; set; }
        public Action Button2_Action { get; set; }

        public event PropertyChangedEventHandler PropertyChanged;
        protected virtual void OnPropertyChanged([CallerMemberName] string propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        private void Window_MouseDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (e.LeftButton == System.Windows.Input.MouseButtonState.Pressed)
            {
                this.DragMove();
            }
        }

        private void MenuItem_Click(object sender, RoutedEventArgs e)
        {
            if (DataGrid.SelectedItem != null && DataGrid.SelectedItem.ToString() != "{NewItemPlaceholder}")
            {
                KeyValueItems.Remove(DataGrid.SelectedItem as KeyValueItem);
            }
        }
    }
}
